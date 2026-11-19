use super::spec::{validate_size, SpawnSpec, MAX_INPUT_BYTES};
use super::PtyError;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::fd::RawFd;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Bounded queue between the PTY reader and the consumer. When full, the reader blocks, the kernel
/// PTY buffer fills, and the child blocks on write: this is the backpressure path.
const EVENT_QUEUE: usize = 64;
const READ_CHUNK: usize = 32 * 1024;
#[cfg(unix)]
const POLL_MS: i32 = 100;
#[cfg(windows)]
const POLL_MS: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    /// Exit code if the process exited normally.
    pub code: Option<u32>,
    /// Signal name if the process was killed by a signal.
    pub signal: Option<String>,
}

#[derive(Debug)]
pub enum SessionEvent {
    Output(Vec<u8>),
    /// Always the last event of a session; all output read before exit is delivered first.
    Exited(ExitInfo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminateOutcome {
    /// Process was already gone; leftover group members were swept.
    AlreadyExited,
    /// Exited after SIGHUP/SIGTERM.
    Graceful,
    /// Required SIGKILL.
    Forced,
}

#[derive(Default)]
struct ExitState {
    info: Mutex<Option<ExitInfo>>,
    cv: Condvar,
}

impl ExitState {
    fn set(&self, info: ExitInfo) {
        *self.info.lock().unwrap() = Some(info);
        self.cv.notify_all();
    }
    fn get(&self) -> Option<ExitInfo> {
        self.info.lock().unwrap().clone()
    }
    fn wait(&self, timeout: Duration) -> Option<ExitInfo> {
        let guard = self.info.lock().unwrap();
        let (guard, _) = self
            .cv
            .wait_timeout_while(guard, timeout, |i| i.is_none())
            .unwrap();
        guard.clone()
    }
}

pub struct PtySession {
    pid: i32,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    exit: Arc<ExitState>,
}

impl PtySession {
    /// Start a process on a new PTY. Returns the session and its event stream.
    /// `on_exit` runs on a background thread once the child has been reaped.
    pub fn spawn(
        spec: &SpawnSpec,
        on_exit: impl FnOnce(&ExitInfo) + Send + 'static,
    ) -> Result<(Arc<Self>, Receiver<SessionEvent>), PtyError> {
        let program = spec.validate()?;

        let mut cmd = CommandBuilder::new(&program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        // Sensible terminal defaults; caller additions override them.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        // The parent must not keep the slave open or EOF is never observed.
        drop(pair.slave);

        let pid = child
            .process_id()
            .ok_or_else(|| PtyError::Spawn("no process id".into()))? as i32;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        // Duplicate fd used only for poll(2), so the reader can notice child exit even when
        // a grandchild still holds the slave side open.
        #[cfg(unix)]
        let poll_fd: RawFd = pair
            .master
            .as_raw_fd()
            .map(|fd| unsafe { libc::dup(fd) })
            .filter(|fd| *fd >= 0)
            .ok_or_else(|| PtyError::Spawn("pty has no fd".into()))?;

        let exit = Arc::new(ExitState::default());
        let (tx, rx) = sync_channel::<SessionEvent>(EVENT_QUEUE);

        // Waiter: reaps the child and publishes its status.
        {
            let exit = exit.clone();
            std::thread::Builder::new()
                .name(format!("pty-wait-{pid}"))
                .spawn(move || {
                    let info = match child.wait() {
                        Ok(st) => ExitInfo {
                            code: st.signal().is_none().then(|| st.exit_code()),
                            signal: st.signal().map(str::to_owned),
                        },
                        Err(_) => ExitInfo {
                            code: None,
                            signal: None,
                        },
                    };
                    on_exit(&info);
                    exit.set(info);
                })?;
        }

        // Reader: the only sender, so Exited is strictly last.
        {
            let exit = exit.clone();
            std::thread::Builder::new()
                .name(format!("pty-read-{pid}"))
                .spawn(move || {
                    #[cfg(unix)]
                    {
                        pump(reader, poll_fd, &tx, &exit);
                        unsafe { libc::close(poll_fd) };
                    }
                    #[cfg(windows)]
                    pump(reader, &tx, &exit);
                    let info = exit.wait(Duration::from_secs(5)).unwrap_or(ExitInfo {
                        code: None,
                        signal: None,
                    });
                    let _ = tx.send(SessionEvent::Exited(info));
                })?;
        }

        let session = Arc::new(Self {
            pid,
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            exit,
        });
        Ok((session, rx))
    }

    pub fn pid(&self) -> u32 {
        self.pid as u32
    }

    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.exit.get()
    }

    pub fn is_running(&self) -> bool {
        self.exit.get().is_none()
    }

    /// Write input bytes (keystrokes, paste) to the PTY. Never silently drops: errors are returned.
    pub fn write_input(&self, data: &[u8]) -> Result<(), PtyError> {
        if data.len() > MAX_INPUT_BYTES {
            return Err(PtyError::Invalid(format!(
                "input larger than {MAX_INPUT_BYTES} bytes"
            )));
        }
        if !self.is_running() {
            return Err(PtyError::Exited);
        }
        let mut w = self.writer.lock().unwrap();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        validate_size(rows, cols)?;
        if !self.is_running() {
            return Err(PtyError::Exited);
        }
        self.master
            .lock()
            .unwrap()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Io(std::io::Error::other(e.to_string())))
    }

    /// Stop the process and its descendants. On Unix: SIGHUP (what closing a terminal does), then
    /// SIGTERM, then SIGKILL, each after a third of `grace`, sent to the session's process group and
    /// to the PTY's current foreground process group; the remaining members are then swept with
    /// SIGKILL. On Windows there is no signal ladder: the process tree is ended with `taskkill /T /F`.
    ///
    /// Limitation: a process that deliberately detaches (setsid/daemonize, or a background job in
    /// its own group that ignores SIGHUP; on Windows a process that reparents itself) can escape; this
    /// is covered by Spike D.
    #[cfg(unix)]
    pub fn terminate(&self, grace: Duration) -> TerminateOutcome {
        let was_running = self.is_running();
        let mut groups = self.groups();
        let mut outcome = TerminateOutcome::AlreadyExited;
        if was_running {
            let step = grace / 3;
            outcome = TerminateOutcome::Graceful;
            for (i, sig) in [libc::SIGHUP, libc::SIGTERM, libc::SIGKILL]
                .into_iter()
                .enumerate()
            {
                for g in self.groups() {
                    if !groups.contains(&g) {
                        groups.push(g);
                    }
                    send_group(g, sig);
                }
                if sig == libc::SIGKILL {
                    outcome = TerminateOutcome::Forced;
                }
                let wait = if i == 2 { Duration::from_secs(2) } else { step };
                if self.exit.wait(wait).is_some() {
                    break;
                }
            }
        }
        for g in groups {
            send_group(g, libc::SIGKILL);
        }
        outcome
    }

    #[cfg(windows)]
    pub fn terminate(&self, grace: Duration) -> TerminateOutcome {
        if !self.is_running() {
            return TerminateOutcome::AlreadyExited;
        }
        crate::platform::kill_process_tree(self.pid as u32);
        let _ = grace; // no graceful step on Windows
        self.exit.wait(Duration::from_secs(2));
        TerminateOutcome::Forced
    }

    /// True if something other than the session's own process (e.g. a command run from an
    /// interactive shell) currently owns the terminal's foreground. Used to decide whether closing
    /// should ask for confirmation. Best effort: false if it cannot be determined.
    #[cfg(unix)]
    pub fn has_foreground_job(&self) -> bool {
        self.is_running()
            && self
                .master
                .lock()
                .unwrap()
                .process_group_leader()
                .is_some_and(|fg| fg > 1 && fg != self.pid)
    }

    /// Windows has no foreground process group; closing never asks for confirmation on that basis.
    #[cfg(windows)]
    pub fn has_foreground_job(&self) -> bool {
        false
    }

    #[cfg(unix)]
    fn groups(&self) -> Vec<i32> {
        let mut v = vec![self.pid];
        if let Some(fg) = self.master.lock().unwrap().process_group_leader() {
            if fg > 1 && fg != self.pid {
                v.push(fg);
            }
        }
        v
    }
}

#[cfg(unix)]
fn send_group(pgid: i32, sig: i32) {
    if pgid > 1 {
        unsafe { libc::killpg(pgid, sig) };
    }
}

#[cfg(unix)]
fn readable(fd: RawFd) -> Option<bool> {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let n = unsafe { libc::poll(&mut p, 1, POLL_MS) };
    if n < 0 {
        return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            Some(false)
        } else {
            None
        };
    }
    if p.revents & libc::POLLNVAL != 0 {
        return None;
    }
    Some(p.revents & (libc::POLLIN | libc::POLLHUP) != 0)
}

#[cfg(unix)]
fn pump(
    mut reader: Box<dyn Read + Send>,
    poll_fd: RawFd,
    tx: &SyncSender<SessionEvent>,
    exit: &ExitState,
) {
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        match readable(poll_fd) {
            None => return,
            Some(true) => match reader.read(&mut buf) {
                Ok(0) | Err(_) => return,
                // Blocks when the consumer is slow: bounded memory + backpressure.
                Ok(n) => {
                    if tx.send(SessionEvent::Output(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
            },
            // Nothing to read. If the child is gone, drain is complete.
            Some(false) => {
                if exit.get().is_some() {
                    return;
                }
            }
        }
    }
}

/// Windows: ConPTY reads cannot be polled, and the read can stay blocked after the child exits. A
/// helper thread does the blocking read and the pump waits on its channel, so it can notice exit.
/// The helper ends when the master is dropped.
#[cfg(windows)]
fn pump(mut reader: Box<dyn Read + Send>, tx: &SyncSender<SessionEvent>, exit: &ExitState) {
    let (chunks, incoming) = sync_channel::<Vec<u8>>(EVENT_QUEUE);
    let _ = std::thread::Builder::new()
        .name("pty-read-blocking".into())
        .spawn(move || {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        if chunks.send(buf[..n].to_vec()).is_err() {
                            return;
                        }
                    }
                }
            }
        });
    loop {
        match incoming.recv_timeout(Duration::from_millis(POLL_MS)) {
            Ok(data) => {
                if tx.send(SessionEvent::Output(data)).is_err() {
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if exit.get().is_some() {
                    // Deliver anything that arrived just before the exit was observed.
                    while let Ok(data) = incoming.try_recv() {
                        if tx.send(SessionEvent::Output(data)).is_err() {
                            return;
                        }
                    }
                    return;
                }
            }
        }
    }
}
