use super::session::{ExitInfo, PtySession, SessionEvent, TerminateOutcome};
use super::{PtyError, SpawnSpec};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

type Registry = Arc<Mutex<HashMap<SessionId, Arc<PtySession>>>>;

/// Registry of live PTY sessions. Sessions remove themselves when their child exits.
#[derive(Default)]
pub struct Supervisor {
    sessions: Registry,
    next: AtomicU64,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&self, spec: &SpawnSpec) -> Result<(SessionId, Receiver<SessionEvent>), PtyError> {
        let id = SessionId(self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let reg = self.sessions.clone();
        // Hold the registry across the spawn. A child that exits at once reports its exit from a
        // background thread; that callback removes the session and must not run before the session has
        // been inserted, or the removal finds nothing and the session stays registered forever.
        let mut sessions = self.sessions.lock().unwrap();
        let (session, rx) = PtySession::spawn(spec, move |_: &ExitInfo| {
            reg.lock().unwrap().remove(&id);
        })?;
        sessions.insert(id, session);
        Ok((id, rx))
    }

    fn get(&self, id: SessionId) -> Result<Arc<PtySession>, PtyError> {
        self.sessions
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(PtyError::NotFound)
    }

    pub fn write(&self, id: SessionId, data: &[u8]) -> Result<(), PtyError> {
        self.get(id)?.write_input(data)
    }

    pub fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Result<(), PtyError> {
        self.get(id)?.resize(rows, cols)
    }

    pub fn terminate(&self, id: SessionId, grace: Duration) -> Result<TerminateOutcome, PtyError> {
        let s = self.get(id)?;
        Ok(s.terminate(grace))
    }

    pub fn has_foreground_job(&self, id: SessionId) -> Result<bool, PtyError> {
        Ok(self.get(id)?.has_foreground_job())
    }

    pub fn list(&self) -> Vec<SessionId> {
        let mut v: Vec<_> = self.sessions.lock().unwrap().keys().copied().collect();
        v.sort();
        v
    }

    /// Terminate every live session (app shutdown). Runs terminations in parallel.
    pub fn shutdown_all(&self, grace: Duration) {
        let all: Vec<_> = self.sessions.lock().unwrap().values().cloned().collect();
        let handles: Vec<_> = all
            .into_iter()
            .map(|s| std::thread::spawn(move || s.terminate(grace)))
            .collect();
        for h in handles {
            let _ = h.join();
        }
    }
}

/// Safety net: dropping the supervisor (e.g. a panic unwinding, or a test failing) must not leave
/// child processes behind. The app also calls `shutdown_all` explicitly with a longer grace.
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.shutdown_all(Duration::from_secs(1));
    }
}
