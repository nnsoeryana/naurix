//! Domain events (ADR 0024, orchestration stage 2). No Tauri dependency (ADR 0002).
//!
//! Events are small metadata notifications: ids, session kind, exit status. They never carry
//! terminal bytes, prompts, titles, environment values or file content. Delivery is bounded and
//! lossy by design (see [`EventBus::publish`]), so consumers must re-read state when they need
//! certainty and nothing may depend on an event for correctness.

use crate::persistence::SessionKind;
use crate::pty::SessionId;
use crate::signals::SignalKind;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Mutex;

/// Events queued per subscriber before new ones are dropped for that subscriber.
pub const SUBSCRIBER_QUEUE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    SessionStarted {
        session: SessionId,
        workspace_id: String,
        kind: SessionKind,
        adapter_id: Option<String>,
    },
    SessionExited {
        session: SessionId,
        workspace_id: String,
        code: Option<u32>,
        signal: Option<String>,
    },
    /// A workflow run or one of its steps changed state (ADR 0030). Ids only; re-read the run for detail.
    RunChanged {
        run_id: String,
        workspace_id: String,
    },
    /// An agent reported its status (ADR 0029). Only the fixed kind and the time; never content.
    SessionStatus {
        session: SessionId,
        workspace_id: String,
        kind: SignalKind,
        at: String,
    },
}

#[derive(Default)]
pub struct EventBus {
    subscribers: Mutex<Vec<SyncSender<Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new receiver that sees every event published from now on. Dropping it unsubscribes.
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = sync_channel(SUBSCRIBER_QUEUE);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    /// Deliver `event` to every subscriber without ever blocking the caller. A subscriber whose
    /// queue is full misses this event; a disconnected subscriber is removed.
    pub fn publish(&self, event: Event) {
        self.subscribers
            .lock()
            .unwrap()
            .retain(|tx| match tx.try_send(event.clone()) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(n: u64) -> Event {
        Event::SessionStarted {
            session: SessionId(n),
            workspace_id: "w".into(),
            kind: SessionKind::Terminal,
            adapter_id: None,
        }
    }

    #[test]
    fn every_subscriber_sees_events_in_order() {
        let bus = EventBus::new();
        let (a, b) = (bus.subscribe(), bus.subscribe());
        bus.publish(started(1));
        bus.publish(started(2));
        for rx in [&a, &b] {
            assert_eq!(rx.try_recv().unwrap(), started(1));
            assert_eq!(rx.try_recv().unwrap(), started(2));
            assert!(rx.try_recv().is_err());
        }
    }

    #[test]
    fn a_dropped_subscriber_is_removed_and_does_not_affect_others() {
        let bus = EventBus::new();
        let keep = bus.subscribe();
        drop(bus.subscribe());
        assert_eq!(bus.subscriber_count(), 2);
        bus.publish(started(1));
        assert_eq!(bus.subscriber_count(), 1);
        assert_eq!(keep.try_recv().unwrap(), started(1));
    }

    #[test]
    fn a_full_subscriber_never_blocks_publishing_and_keeps_its_queue() {
        let bus = EventBus::new();
        let slow = bus.subscribe();
        let live = bus.subscribe();
        for i in 0..(SUBSCRIBER_QUEUE as u64 + 10) {
            bus.publish(started(i));
            // `live` keeps up; `slow` never reads.
            assert_eq!(live.try_recv().unwrap(), started(i));
        }
        assert_eq!(
            bus.subscriber_count(),
            2,
            "a full queue is not a disconnect"
        );
        let queued = std::iter::from_fn(|| slow.try_recv().ok()).count();
        assert_eq!(queued, SUBSCRIBER_QUEUE);
    }
}
