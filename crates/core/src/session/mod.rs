//! Session ownership (ADR 0023, 0026). No Tauri dependency (ADR 0002).
//!
//! A session is a process on a PTY plus its metadata row. [`SessionManager`] owns the process
//! supervisor, the per-session flow-control state, the workspace association and the link to the
//! database row. A session's lifetime belongs to Core, not to a view (ADR 0026): a consumer
//! (the UI) can [`attach`](SessionManager::attach) and [`detach`](SessionManager::detach) without
//! affecting the process, and only [`close`](SessionManager::close) or application quit ends it.
//!
//! Output is always drained into a bounded **in-memory** ring buffer, so a detached session never
//! blocks its child, and a consumer that attaches later gets the tail replayed. The ring is never
//! persisted or logged and is dropped on close. Terminal bytes otherwise pass through untouched.

use crate::events::{Event, EventBus};
use crate::persistence::{iso8601_utc_now, Db, DbError, SessionKind};
use crate::pty::{
    ExitInfo, Flow, Outbound, PtyError, SessionEvent, SessionId, SpawnSpec, Supervisor,
    TerminateOutcome, MAX_BATCH_BYTES,
};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Replay buffer per session. A hypothesis (ADR 0026, ADR 0006): confirm with a recorded benchmark
/// before documenting any memory figure.
pub const DEFAULT_REPLAY_BYTES: usize = 1 << 20;
/// Exited sessions kept for replay until closed. Bounds memory if a consumer never calls `close`.
pub const MAX_EXITED_RETAINED: usize = 16;
const MAX_TITLE_CHARS: usize = 100;
/// Agents that may run at once until the setting is applied (ADR 0027).
pub const DEFAULT_MAX_AGENTS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("{0}")]
    Db(#[from] DbError),
    #[error("The workspace folder is missing: {0}")]
    MissingFolder(String),
    #[error("session not found")]
    NotFound,
    #[error("{0}")]
    Pty(#[from] PtyError),
    #[error("{0}")]
    Thread(String),
    #[error("{}", agent_limit_message(*running, *limit))]
    AgentLimit { running: usize, limit: usize },
}

fn agent_limit_message(running: usize, limit: usize) -> String {
    let (noun, verb) = if running == 1 {
        ("agent", "is")
    } else {
        ("agents", "are")
    };
    format!("{running} {noun} {verb} already running (the limit is {limit}). Stop one, or raise the limit in Settings, AI Agents.")
}

/// What to start in a new session. Built only by the backend from validated data, never by the webview.
#[derive(Debug, Clone)]
pub struct SessionLaunch {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub kind: SessionKind,
    pub title: String,
    pub adapter_id: Option<String>,
    /// The approval that allowed this launch, recorded on the session row for traceability (ADR 0027).
    pub approval_id: Option<String>,
    /// Start somewhere other than the workspace folder. Only ever set by the backend.
    pub cwd: Option<PathBuf>,
}

/// Metadata about a session (never output). Returned by [`SessionManager::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    pub workspace_id: String,
    pub kind: SessionKind,
    pub title: String,
    pub adapter_id: Option<String>,
    pub started_at: String,
    /// `None` while the process is running.
    pub exit: Option<ExitInfo>,
}

impl SessionInfo {
    pub fn running(&self) -> bool {
        self.exit.is_none()
    }
}

type Sink = Box<dyn FnMut(Outbound) -> bool + Send>;

/// Everything a consumer can observe, guarded by one lock so that "append to the ring and send to
/// the sink" and "snapshot the ring and install the sink" are each atomic (no gap, no duplicate).
struct Output {
    ring: VecDeque<u8>,
    limit: usize,
    sink: Option<Sink>,
    exit: Option<ExitInfo>,
    /// Identifies the current consumer. The consumer given to `start` is generation 0; every
    /// `attach` increments it, so a late `detach_if` from an older consumer cannot drop a newer one.
    generation: u64,
}

impl Output {
    fn push(&mut self, data: &[u8]) {
        if data.len() >= self.limit {
            self.ring.clear();
            self.ring.extend(&data[data.len() - self.limit..]);
        } else {
            self.ring.extend(data);
            let over = self.ring.len().saturating_sub(self.limit);
            self.ring.drain(..over);
        }
    }
}

struct Entry {
    flow: Arc<Flow>,
    workspace_id: String,
    kind: SessionKind,
    title: String,
    adapter_id: Option<String>,
    started_at: String,
    out: Mutex<Output>,
}

impl Entry {
    /// Record `batch` in the ring and, if a consumer is attached, send it on.
    fn deliver(&self, batch: Vec<u8>) {
        // Only wait for the consumer while one is attached; a detached session just drains.
        let attached = self.out.lock().unwrap().sink.is_some();
        if attached {
            // Returns false when the flow was closed (detach or shutdown): fall through.
            let _ = self.flow.wait_below_high_water();
        }
        let mut o = self.out.lock().unwrap();
        o.push(&batch);
        let n = batch.len();
        if let Some(sink) = o.sink.as_mut() {
            self.flow.add(n);
            if !sink(Outbound::Data(batch)) {
                // The consumer is gone. The session keeps running and keeps draining into the ring.
                o.sink = None;
            }
        }
    }

    fn finish(&self, info: ExitInfo) {
        let mut o = self.out.lock().unwrap();
        o.exit = Some(info.clone());
        if let Some(sink) = o.sink.as_mut() {
            if !sink(Outbound::Exited(info)) {
                o.sink = None;
            }
        }
    }

    fn info(&self, id: SessionId) -> SessionInfo {
        SessionInfo {
            id,
            workspace_id: self.workspace_id.clone(),
            kind: self.kind,
            title: self.title.clone(),
            adapter_id: self.adapter_id.clone(),
            started_at: self.started_at.clone(),
            exit: self.out.lock().unwrap().exit.clone(),
        }
    }

    fn drop_consumer(&self) {
        self.out.lock().unwrap().sink = None;
        self.flow.close();
    }
}

pub struct SessionManager {
    sup: Supervisor,
    db: Arc<Db>,
    grace: Duration,
    replay_limit: usize,
    max_agents: AtomicUsize,
    /// Held for the whole start of an agent so the limit check and the registration are one step.
    agent_start: Mutex<()>,
    entries: Mutex<HashMap<SessionId, Arc<Entry>>>,
    events: Arc<EventBus>,
}

/// A title the database accepts: trimmed, no control characters, at most 100 characters.
fn sanitize_title(t: &str) -> String {
    let s: String = t
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_CHARS + 50)
        .collect();
    let s: String = s.trim().chars().take(MAX_TITLE_CHARS).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "Session".into()
    } else {
        s
    }
}

impl SessionManager {
    /// `grace` is the time a terminating session gets before it is force-killed (see `PtySession::terminate`).
    pub fn new(db: Arc<Db>, grace: Duration) -> Self {
        Self {
            sup: Supervisor::new(),
            db,
            grace,
            replay_limit: DEFAULT_REPLAY_BYTES,
            max_agents: AtomicUsize::new(DEFAULT_MAX_AGENTS),
            agent_start: Mutex::new(()),
            entries: Mutex::new(HashMap::new()),
            events: Arc::new(EventBus::new()),
        }
    }

    /// Replay buffer size per session (default [`DEFAULT_REPLAY_BYTES`]). Applies to sessions started afterwards.
    pub fn with_replay_limit(mut self, bytes: usize) -> Self {
        self.replay_limit = bytes.max(1);
        self
    }

    /// Most agent sessions that may run at once. Lowering it never stops running agents; it only
    /// refuses new ones. Plain terminals are never limited.
    pub fn set_max_agents(&self, n: usize) {
        self.max_agents.store(n.max(1), Ordering::Relaxed);
    }

    /// Agent sessions whose process is still running.
    pub fn running_agents(&self) -> usize {
        let all: Vec<Arc<Entry>> = self.entries.lock().unwrap().values().cloned().collect();
        all.iter()
            .filter(|e| e.kind == SessionKind::Agent && e.out.lock().unwrap().exit.is_none())
            .count()
    }

    /// Fail if no agent slot is free. A quick look for callers that want to refuse before doing
    /// anything irreversible (such as spending a one-time approval); `start` checks again atomically.
    pub fn ensure_agent_slot(&self) -> Result<(), SessionError> {
        let (running, limit) = (
            self.running_agents(),
            self.max_agents.load(Ordering::Relaxed),
        );
        if running >= limit {
            return Err(SessionError::AgentLimit { running, limit });
        }
        Ok(())
    }

    /// Domain events about sessions (ADR 0024). Metadata only; delivery is lossy by design.
    pub fn events(&self) -> &Arc<EventBus> {
        &self.events
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }

    /// Start a PTY session in a workspace, record its metadata, and send its output to `sink`
    /// (the initial consumer; replace or remove it later with `attach`/`detach`). The session runs
    /// until its process exits; it is forgotten only by [`close`](Self::close) or quit.
    ///
    /// Fails, and stops the process it just started, if its metadata row cannot be recorded or
    /// its forwarding thread cannot start (ADR 0026): a session is never left running unmanaged.
    pub fn start(
        self: &Arc<Self>,
        workspace_id: &str,
        launch: SessionLaunch,
        rows: u16,
        cols: u16,
        sink: impl FnMut(Outbound) -> bool + Send + 'static,
    ) -> Result<SessionId, SessionError> {
        // Agents are counted and registered under one lock, so two launches cannot both take the last slot.
        let _slot = (launch.kind == SessionKind::Agent).then(|| self.agent_start.lock().unwrap());
        if launch.kind == SessionKind::Agent {
            self.ensure_agent_slot()?;
        }
        let ws = self.db.workspace(workspace_id)?;
        let cwd = launch.cwd.unwrap_or_else(|| PathBuf::from(&ws.root_path));
        if !cwd.is_dir() {
            return Err(SessionError::MissingFolder(ws.root_path));
        }
        let spec = SpawnSpec {
            program: launch.program,
            args: launch.args,
            cwd,
            env: launch.env,
            rows,
            cols,
        };
        let (id, rx) = self.sup.spawn(&spec)?;
        let title = sanitize_title(&launch.title);
        let db_id = match self.db.insert_session(
            &ws.id,
            launch.kind,
            &title,
            launch.adapter_id.as_deref(),
            launch.approval_id.as_deref(),
        ) {
            Ok(d) => d,
            Err(e) => {
                let _ = self.sup.terminate(id, self.grace);
                return Err(e.into());
            }
        };
        let entry = Arc::new(Entry {
            flow: Arc::new(Flow::new()),
            workspace_id: ws.id.clone(),
            kind: launch.kind,
            title,
            adapter_id: launch.adapter_id.clone(),
            started_at: iso8601_utc_now(),
            out: Mutex::new(Output {
                ring: VecDeque::new(),
                limit: self.replay_limit,
                sink: Some(Box::new(sink)),
                exit: None,
                generation: 0,
            }),
        });

        // The forwarder waits at this gate so `SessionStarted` is always published before
        // `SessionExited`, even for a process that exits immediately.
        let (gate_tx, gate_rx) = channel::<()>();
        let hub = self.clone();
        let fwd_entry = entry.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("forward-{id}"))
            .spawn(move || {
                let _ = gate_rx.recv();
                run_forwarder(&hub, id, &fwd_entry, &rx, &db_id);
            });
        if let Err(e) = spawned {
            let _ = self.sup.terminate(id, self.grace);
            return Err(SessionError::Thread(e.to_string()));
        }
        self.entries.lock().unwrap().insert(id, entry);
        self.events.publish(Event::SessionStarted {
            session: id,
            workspace_id: ws.id,
            kind: launch.kind,
            adapter_id: launch.adapter_id,
        });
        let _ = gate_tx.send(());
        Ok(id)
    }

    fn entry(&self, id: SessionId) -> Result<Arc<Entry>, SessionError> {
        self.entries
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(SessionError::NotFound)
    }

    /// Every session that has not been closed: running ones and exited ones kept for replay.
    /// Metadata only, ordered by id.
    pub fn list(&self) -> Vec<SessionInfo> {
        let all: Vec<(SessionId, Arc<Entry>)> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .map(|(id, e)| (*id, e.clone()))
            .collect();
        let mut v: Vec<SessionInfo> = all.iter().map(|(id, e)| e.info(*id)).collect();
        v.sort_by_key(|i| i.id);
        v
    }

    /// Make `sink` the consumer of this session, replacing any previous one. The buffered tail of
    /// the output is replayed first, then the exit status if the process already ended, then live
    /// output follows with no gap and no duplicate. Flow control restarts for the new consumer.
    /// Returns the consumer's generation, to be passed to [`detach_if`](Self::detach_if).
    pub fn attach(
        &self,
        id: SessionId,
        mut sink: impl FnMut(Outbound) -> bool + Send + 'static,
    ) -> Result<u64, SessionError> {
        let e = self.entry(id)?;
        let mut o = e.out.lock().unwrap();
        e.flow.reopen();
        o.sink = None;
        o.generation += 1;
        let token = o.generation;
        let replay: Vec<u8> = o.ring.iter().copied().collect();
        for chunk in replay.chunks(MAX_BATCH_BYTES) {
            e.flow.add(chunk.len());
            if !sink(Outbound::Data(chunk.to_vec())) {
                return Ok(token);
            }
        }
        if let Some(exit) = &o.exit {
            if !sink(Outbound::Exited(exit.clone())) {
                return Ok(token);
            }
        }
        o.sink = Some(Box::new(sink));
        Ok(token)
    }

    /// Stop delivering output to the current consumer. The process keeps running and keeps draining
    /// into the replay buffer.
    pub fn detach(&self, id: SessionId) -> Result<(), SessionError> {
        self.entry(id)?.drop_consumer();
        Ok(())
    }

    /// Like [`detach`](Self::detach), but only if the consumer with this generation is still the
    /// current one (0 is the consumer passed to `start`). A stale detach from a consumer that has
    /// already been replaced does nothing. Returns whether a consumer was dropped.
    pub fn detach_if(&self, id: SessionId, generation: u64) -> Result<bool, SessionError> {
        let e = self.entry(id)?;
        if e.out.lock().unwrap().generation != generation {
            return Ok(false);
        }
        e.drop_consumer();
        Ok(true)
    }

    /// Drop every consumer; processes keep running. Used when the webview reloads, because the
    /// consumers it held no longer exist.
    pub fn detach_all(&self) {
        let all: Vec<Arc<Entry>> = self.entries.lock().unwrap().values().cloned().collect();
        for e in all {
            e.drop_consumer();
        }
    }

    /// Stop the process if it is running and forget the session, freeing its replay buffer.
    /// Blocks for up to the grace period plus 2 s; call it off an async worker.
    pub fn close(&self, id: SessionId) -> Result<(), SessionError> {
        let e = self
            .entries
            .lock()
            .unwrap()
            .remove(&id)
            .ok_or(SessionError::NotFound)?;
        e.drop_consumer();
        e.out.lock().unwrap().ring = VecDeque::new();
        // Already-exited sessions are no longer in the supervisor; that is fine.
        let _ = self.sup.terminate(id, self.grace);
        Ok(())
    }

    pub fn write(&self, id: SessionId, data: &[u8]) -> Result<(), SessionError> {
        Ok(self.sup.write(id, data)?)
    }

    pub fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Result<(), SessionError> {
        Ok(self.sup.resize(id, rows, cols)?)
    }

    /// The consumer finished processing `bytes` of output (flow control).
    pub fn ack(&self, id: SessionId, bytes: usize) -> Result<(), SessionError> {
        self.entry(id)?.flow.ack(bytes);
        Ok(())
    }

    /// Whether a command owns the terminal's foreground. Best effort.
    pub fn has_foreground_job(&self, id: SessionId) -> Result<bool, SessionError> {
        Ok(self.sup.has_foreground_job(id)?)
    }

    /// Stop the process but keep the session (and its replay buffer) until `close`.
    /// Blocks for up to the grace period plus 2 s; call it off an async worker.
    pub fn terminate(&self, id: SessionId) -> Result<TerminateOutcome, SessionError> {
        Ok(self.sup.terminate(id, self.grace)?)
    }

    /// Stop and forget every session of one workspace (used when the workspace is removed).
    pub fn terminate_workspace(&self, workspace_id: &str) {
        let ids: Vec<SessionId> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| e.workspace_id == workspace_id)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let _ = self.close(id);
        }
    }

    /// Terminate every session (application quit).
    pub fn shutdown(&self) {
        let all: Vec<Arc<Entry>> = self
            .entries
            .lock()
            .unwrap()
            .drain()
            .map(|(_, e)| e)
            .collect();
        for e in all {
            e.drop_consumer();
        }
        self.sup.shutdown_all(self.grace);
    }

    /// Running sessions that quitting would interrupt: agents, and terminals with a command in the
    /// foreground. An idle shell does not count. Best effort, like `has_foreground_job`.
    pub fn busy_count(&self) -> usize {
        self.list()
            .iter()
            .filter(|i| {
                i.running()
                    && (i.kind == SessionKind::Agent
                        || self.sup.has_foreground_job(i.id).unwrap_or(false))
            })
            .count()
    }

    /// Number of live processes (not counting exited sessions kept for replay).
    pub fn session_count(&self) -> usize {
        self.sup.list().len()
    }

    /// Keep at most [`MAX_EXITED_RETAINED`] exited sessions, forgetting the oldest first.
    fn prune_exited(&self) {
        let all: Vec<(SessionId, Arc<Entry>)> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .map(|(id, e)| (*id, e.clone()))
            .collect();
        let mut exited: Vec<SessionId> = all
            .iter()
            .filter(|(_, e)| e.out.lock().unwrap().exit.is_some())
            .map(|(id, _)| *id)
            .collect();
        if exited.len() <= MAX_EXITED_RETAINED {
            return;
        }
        exited.sort();
        let drop_n = exited.len() - MAX_EXITED_RETAINED;
        let mut map = self.entries.lock().unwrap();
        for id in &exited[..drop_n] {
            if let Some(e) = map.remove(id) {
                e.out.lock().unwrap().ring = VecDeque::new();
            }
        }
    }
}

/// Runs until the process exits: batches queued chunks (as `pty::forward` does), hands them to the
/// entry, then publishes the exit and finishes the metadata row.
fn run_forwarder(
    hub: &Arc<SessionManager>,
    id: SessionId,
    entry: &Arc<Entry>,
    rx: &Receiver<SessionEvent>,
    db_id: &str,
) {
    let info = loop {
        let mut batch = match rx.recv() {
            Ok(SessionEvent::Output(b)) => b,
            Ok(SessionEvent::Exited(i)) => break i,
            Err(_) => {
                break ExitInfo {
                    code: None,
                    signal: None,
                }
            }
        };
        let mut exited = None;
        while batch.len() < MAX_BATCH_BYTES {
            match rx.try_recv() {
                Ok(SessionEvent::Output(b)) => batch.extend_from_slice(&b),
                Ok(SessionEvent::Exited(i)) => {
                    exited = Some(i);
                    break;
                }
                Err(_) => break,
            }
        }
        entry.deliver(batch);
        if let Some(i) = exited {
            break i;
        }
    };
    entry.finish(info.clone());
    hub.events.publish(Event::SessionExited {
        session: id,
        workspace_id: entry.workspace_id.clone(),
        code: info.code,
        signal: info.signal,
    });
    let _ = hub.db.finish_session(db_id, "exited");
    hub.prune_exited();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(limit: usize) -> Output {
        Output {
            ring: VecDeque::new(),
            limit,
            sink: None,
            exit: None,
            generation: 0,
        }
    }

    #[test]
    fn the_ring_keeps_only_the_newest_bytes() {
        let mut o = out(8);
        o.push(b"abcde");
        o.push(b"fghij");
        assert_eq!(o.ring.iter().copied().collect::<Vec<_>>(), b"cdefghij");
        o.push(b"0123456789ABCDEF"); // larger than the limit by itself
        assert_eq!(o.ring.iter().copied().collect::<Vec<_>>(), b"89ABCDEF");
    }

    #[test]
    fn titles_are_made_acceptable_instead_of_failing_the_start() {
        assert_eq!(sanitize_title("  hi\n\x1b[0m there "), "hi[0m there");
        assert_eq!(sanitize_title("   "), "Session");
        assert_eq!(sanitize_title(&"x".repeat(300)).chars().count(), 100);
    }
}
