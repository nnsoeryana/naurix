//! Forwarding of session output to a consumer (the webview) with batching and ack-based flow
//! control. Terminal bytes pass through; nothing is stored or logged.

use super::session::{ExitInfo, SessionEvent};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Condvar, Mutex};

/// Largest batch sent in one message.
pub const MAX_BATCH_BYTES: usize = 64 * 1024;
/// Forwarding pauses while this many bytes are unacknowledged by the consumer.
pub const HIGH_WATER_BYTES: usize = 256 * 1024;

#[derive(Debug)]
pub enum Outbound {
    Data(Vec<u8>),
    Exited(ExitInfo),
}

#[derive(Default)]
struct FlowState {
    in_flight: usize,
    closed: bool,
}

/// Tracks bytes sent but not yet acknowledged by the consumer.
#[derive(Default)]
pub struct Flow {
    state: Mutex<FlowState>,
    cv: Condvar,
}

impl Flow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumer finished processing `n` bytes.
    pub fn ack(&self, n: usize) {
        let mut s = self.state.lock().unwrap();
        s.in_flight = s.in_flight.saturating_sub(n);
        self.cv.notify_all();
    }

    /// Stop forwarding and wake any waiter (session closed or consumer gone).
    pub fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.cv.notify_all();
    }

    pub fn in_flight(&self) -> usize {
        self.state.lock().unwrap().in_flight
    }

    /// Block while at/above the high-water mark. Returns false if the flow was closed.
    fn wait_below_high_water(&self) -> bool {
        let s = self.state.lock().unwrap();
        let s = self
            .cv
            .wait_while(s, |s| s.in_flight >= HIGH_WATER_BYTES && !s.closed)
            .unwrap();
        !s.closed
    }

    fn add(&self, n: usize) {
        self.state.lock().unwrap().in_flight += n;
    }
}

/// Pump events from a session to `sink` until the session exits, the sink refuses (returns
/// false), or the flow is closed. Queued chunks are coalesced into batches of at most
/// [`MAX_BATCH_BYTES`] without waiting, so an idle terminal echoes keystrokes immediately and a
/// flooding one is sent in large batches.
pub fn forward(rx: &Receiver<SessionEvent>, flow: &Flow, mut sink: impl FnMut(Outbound) -> bool) {
    loop {
        let first = match rx.recv() {
            Ok(e) => e,
            Err(_) => return,
        };
        let mut batch = match first {
            SessionEvent::Exited(info) => {
                sink(Outbound::Exited(info));
                return;
            }
            SessionEvent::Output(b) => b,
        };
        let mut exited = None;
        while batch.len() < MAX_BATCH_BYTES {
            match rx.try_recv() {
                Ok(SessionEvent::Output(b)) => batch.extend_from_slice(&b),
                Ok(SessionEvent::Exited(info)) => {
                    exited = Some(info);
                    break;
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        if !flow.wait_below_high_water() {
            return;
        }
        flow.add(batch.len());
        if !sink(Outbound::Data(batch)) {
            return;
        }
        if let Some(info) = exited {
            sink(Outbound::Exited(info));
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;
    use std::sync::Arc;
    use std::time::Duration;

    fn exit() -> ExitInfo {
        ExitInfo {
            code: Some(0),
            signal: None,
        }
    }

    #[test]
    fn coalesces_queued_chunks_and_keeps_exit_last() {
        let (tx, rx) = sync_channel(16);
        for _ in 0..3 {
            tx.send(SessionEvent::Output(vec![1; 10])).unwrap();
        }
        tx.send(SessionEvent::Exited(exit())).unwrap();
        let flow = Flow::new();
        let mut got = vec![];
        forward(&rx, &flow, |o| {
            got.push(o);
            true
        });
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[0], Outbound::Data(d) if d.len() == 30));
        assert!(matches!(&got[1], Outbound::Exited(_)));
        assert_eq!(flow.in_flight(), 30);
    }

    #[test]
    fn batches_are_capped() {
        let (tx, rx) = sync_channel(16);
        for _ in 0..4 {
            tx.send(SessionEvent::Output(vec![0; 40 * 1024])).unwrap();
        }
        drop(tx);
        let flow = Flow::new();
        let mut sizes = vec![];
        forward(&rx, &flow, |o| {
            if let Outbound::Data(d) = o {
                sizes.push(d.len());
                flow.ack(d.len());
            }
            true
        });
        assert!(
            sizes.iter().all(|s| *s <= MAX_BATCH_BYTES + 40 * 1024),
            "{sizes:?}"
        );
        assert_eq!(sizes.iter().sum::<usize>(), 160 * 1024);
        assert!(sizes.len() >= 2);
    }

    #[test]
    fn pauses_at_high_water_until_acked() {
        let (tx, rx) = sync_channel(64);
        let flow = Arc::new(Flow::new());
        let sent = Arc::new(Mutex::new(0usize));
        let (f, s) = (flow.clone(), sent.clone());
        let h = std::thread::spawn(move || {
            forward(&rx, &f, |o| {
                if let Outbound::Data(d) = o {
                    *s.lock().unwrap() += d.len();
                }
                true
            })
        });
        for _ in 0..40 {
            tx.send(SessionEvent::Output(vec![0; MAX_BATCH_BYTES]))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(200));
        let before = *sent.lock().unwrap();
        assert!(
            before <= HIGH_WATER_BYTES + MAX_BATCH_BYTES,
            "sent {before} without acks"
        );
        assert!(before >= HIGH_WATER_BYTES, "sent only {before}");
        flow.ack(before);
        std::thread::sleep(Duration::from_millis(200));
        assert!(*sent.lock().unwrap() > before, "did not resume after ack");
        flow.close();
        drop(tx);
        h.join().unwrap();
    }

    #[test]
    fn close_unblocks_a_stalled_forwarder() {
        let (tx, rx) = sync_channel(64);
        let flow = Arc::new(Flow::new());
        let f = flow.clone();
        let h = std::thread::spawn(move || forward(&rx, &f, |_| true));
        for _ in 0..8 {
            tx.send(SessionEvent::Output(vec![0; MAX_BATCH_BYTES]))
                .unwrap();
        }
        std::thread::sleep(Duration::from_millis(100));
        flow.close();
        h.join().unwrap();
    }

    #[test]
    fn stops_when_sink_refuses() {
        let (tx, rx) = sync_channel(4);
        tx.send(SessionEvent::Output(vec![1])).unwrap();
        tx.send(SessionEvent::Output(vec![2])).unwrap();
        drop(tx);
        let flow = Flow::new();
        let mut calls = 0;
        forward(&rx, &flow, |_| {
            calls += 1;
            false
        });
        assert_eq!(calls, 1);
    }
}
