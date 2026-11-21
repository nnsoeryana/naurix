# ADR 0026 — Sessions outlive the view: detach, reattach and window close (orchestration stage 4)
**Status:** Proposed (maintainer to accept; the four open decisions were answered on 2026-11-21, see the end). **Design only; no code.** Implements the session-lifecycle decisions of ADR 0022 §2 and resolves the known quirks recorded in ADR 0023 and ADR 0024. This is the riskiest stage: it changes who owns a session's lifetime.

## Context (from the code, 2026-11-21)
- **Session lifetime is bound to the React component.** `TerminalView` spawns its session on mount and calls `terminate` on unmount (`src/features/terminal/TerminalView.tsx`). Closing a pane, closing a tab and unmounting for any other reason all kill the process.
- **A page load kills every session.** `on_page_load(Started)` calls `SessionManager::shutdown()` (`src-tauri/src/lib.rs`).
- **The UI cannot rediscover sessions.** Tabs, splits and the pane-to-session map live only in Zustand (ADR 0010). Core has no way to list live sessions.
- **Output has exactly one consumer, fixed at start.** The forwarder thread owns the sink closure; when the sink refuses output the thread stops, the entry is removed and the session row is marked `exited` even though the process may still be running (ADR 0023 quirk). If the thread cannot start, the process keeps running unmanaged. If the `sessions` row cannot be inserted, the session runs without metadata.
- `SessionExited` is not published if the sink refuses first or `shutdown` runs (ADR 0024 limit).
- There is **no window-close handling** anywhere (`on_window_event`, `ExitRequested` and `Reopen` do not appear in `src-tauri`). Tauri 2.12.7 and tao 0.37.3 are in use. My understanding is that Tauri exits when the last window closes unless prevented; **this must be confirmed against these versions before implementation** and is not asserted here.

## Decision

### 1. Core owns the session lifetime; the UI only attaches
- A session lives until its process exits **and** it is explicitly **closed**, or the application quits. No UI event (reload, unmount, window hide) ends a session.
- New Core operations on `SessionManager`: `list()` (metadata of every live or exited-but-unclosed session: wire id, workspace id, kind, title, adapter id, started time, running or exited with code/signal), `attach(id, sink)`, `detach(id)`, and `close(id)` (terminate if running, then forget the session and free its buffer). `terminate(id)` stays as "stop the process" without forgetting it.
- The wire session id stays the runtime `u64` (live sessions only; after an application restart none exist). The database row id is held in the session entry so `list()` can join the stored title. A single identifier across runtime and database is not needed for this stage.

### 2. Output path: drain always, replay on attach
- The forwarder thread runs **until the process exits**, regardless of whether a view is attached. It always drains the PTY into a **bounded in-memory ring buffer** per session (drop oldest). A detached session therefore never blocks its child on a full PTY.
- While a sink is attached, new output is also sent to it with the existing batching and ack-based flow control. If the sink refuses, the session **detaches itself** and keeps running (this removes the ADR 0023 quirk where a refusing sink ended the forwarder).
- `attach` takes the session's output lock, sends the ring contents to the new sink as replay, then installs the sink and resets flow-control state, all under that lock so no byte is lost or duplicated between replay and live output. One consumer at a time: a second attach replaces the first.
- **The ring is memory only.** It is never written to the database, a file or a log, is cleared on `close`, and is dropped when the application exits. This is the same class of data as xterm's in-memory scrollback (TERM-009) and does not change DATA-003. Its size is a hypothesis (starting value 1 MiB per session) to be set from a recorded benchmark with several agents before any number is documented (ADR 0006).
- **Known limitation, stated up front:** replaying the tail of a byte stream into a fresh terminal can render full-screen TUIs (agent CLIs) incorrectly if the cut falls inside an escape sequence or the alternate-screen switch is missing. Mitigation: after replay the UI nudges the PTY size (resize by one then back) so TUIs that redraw on `SIGWINCH` repaint. The result is best effort and must be tried with each real agent CLI before this stage is called done. A server-side terminal emulator in Core that snapshots the screen would be exact but adds a dependency and substantial code; it is **not** chosen here and can be revisited by a new ADR if replay proves inadequate.

### 3. UI reload (all platforms)
- Reload no longer terminates anything: remove the shutdown in `on_page_load`.
- On startup `init()` asks Core for `list()`. Each live or exited-but-unclosed session reappears as **one tab in its workspace** and attaches with replay. **Split layout is not restored** (layout stays UI-only state per ADR 0010); persisting layout is out of scope.
- Sessions that no view attaches to are still listed, so a leaked session is always visible and stoppable, and the existing session count in the performance snapshot stays meaningful.
- Component cleanup changes meaning: an **unmount only detaches**. Killing a process happens only on an explicit user action (close pane, close tab, remove workspace, quit), which calls `close`. This inverts the current responsibility and is the main frontend change; every unmount path must be audited so none still terminates (including development hot reload and React StrictMode double-mount).

### 4. Window close and quit
- **macOS (the only supported platform, ADR 0001):** the red close button **hides** the window and the process keeps running with its sessions; clicking the Dock icon (`Reopen`) shows it again. Hiding keeps the webview alive, so closing the window needs no reattach at all; only reload does. Cmd+Q and the Quit menu item quit.
- **Quit** terminates all sessions gracefully with the existing ladder (ADR 0022 §2.3). The existing exit hook (`RunEvent::Exit`) stays. **Recommended:** if sessions are running and "Ask before closing running work" is on, quit asks first, because sessions can now outlive the window and the user may not realise the app is still running. Confirmed by the maintainer (see the end).
- **Linux and Windows:** the window-close behaviour is **unchanged** (closing the last window quits) until each platform is validated on real hardware (ADR 0021). Reload survival is Core-level and applies everywhere.
- **Crash or kill of Naurix:** children are expected to receive `SIGHUP` when the PTY master closes with the process; there is no daemon to rely on. This must be verified by a Spike D equivalent test (kill the app process, assert no orphans) before the behaviour is documented as a guarantee.

### 5. Fixes folded in (previously recorded quirks)
- `SessionExited` is published whenever the process exits, because the forwarder no longer stops early (ADR 0024 limit). It is still not published when the application quits through `shutdown`.
- If inserting the `sessions` row fails, the start **fails**: the just-spawned process is terminated and an error returned, instead of running without metadata.
- If the forwarder thread cannot be started, the process is terminated and the start fails.
- The session row is marked `exited` only when the process has actually exited, not when a consumer disappears.

### 6. Security and privacy
- `list`, `attach`, `detach`, `close` take only a numeric id; the webview still cannot choose a program, a directory or an environment. Listing returns metadata, never output.
- Output is readable only through `attach` by the application's own webview, the same trust level as today.
- Nothing is persisted that was not persisted before: the replay ring is memory only and bounded.

## Consequences
- **Frontend** (`TerminalView`, `store.ts`, `terminal/api.ts`, `agents/api.ts`) and **Tauri commands** (`session_list`, `session_attach`, `session_detach`, `session_close`, plus window and exit handlers in `lib.rs`) change in the implementation stage. Core changes are in `session/mod.rs` and `pty/forward.rs` (a swappable sink and a ring buffer).
- After acceptance, PRD gains SES-005 (detached sessions drain into a bounded in-memory buffer that is never persisted; replay is best effort) and SES-006 (sessions without an attached view remain listed and stoppable), and AGENTS.md's Sessions invariant gains the "unmount detaches, only explicit close ends a session" rule.
- Tests required before the stage is done: Core tests for attach/detach/reattach with replay (no gap, no duplicate), a detached flooding child that is not blocked, exit while detached, attach to an exited session, `close`, quit with detached sessions, start failure paths; a Spike D equivalent for kill-the-app orphans; a manual checklist on macOS for reload, window hide and Dock reopen, quit confirmation, and each real agent CLI repainting after replay; and a recorded memory benchmark for N sessions with the ring.
- Risk: sessions can run without a visible window, and a UI bug could leak sessions. Mitigations: `list()` shows everything, quit terminates everything, and the quit confirmation recommended above.
- Risk: replay renders badly for some TUIs. Mitigation: the size nudge, per-agent manual verification, and the emulator-in-Core option kept open.

## Decided by the maintainer (2026-11-21)
1. **macOS window close hides the window** (the process and sessions keep running; Dock click reopens; Cmd+Q quits). Destroy-and-recreate was rejected.
2. **Quit asks for confirmation when sessions are running**, following the existing "Ask before closing running work" setting.
3. **Replay buffer starts at 1 MiB per session.** This is a hypothesis; the final size and any memory figure need a recorded benchmark with several agents (ADR 0006).
4. **After a reload each session returns as one tab without splits.** Persisting layout is out of scope.

Still open for the implementation stage: confirming Tauri 2.12.7's last-window-close behaviour and the `Reopen` event (§4), and the per-agent replay check (§2).

## Implementation log
**Part (a), Core, 2026-11-21.** `crates/core/src/session/mod.rs` (rewritten), `Flow::reopen` in `pty/forward.rs`, tests in `crates/core/tests/session.rs` (one assertion updated) and the new `session_reattach.rs` (7 tests). Verified with `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, and the session/event tests repeated 15 times without a failure. `src-tauri` and the frontend are **not changed**, so the running app still behaves as before (a page load still shuts sessions down) until part (b). The forwarder, `attach`, `detach`, `close` and `list` exist and are tested but nothing in the UI calls them yet.

Two decisions made while implementing, not in the first draft:
1. **Exited sessions are retained up to a cap** (`MAX_EXITED_RETAINED` = 16, oldest forgotten first), in addition to explicit `close`. The current UI never calls `close`, and any future UI bug could do the same; without a cap each exited session would hold up to 1 MiB until quit.
2. **Titles are sanitised instead of failing the start.** The database rejects titles over 100 characters or with control characters, and the terminal title is derived from the folder path, so a deep path would have made "start fails if the row cannot be recorded" break ordinary terminals. The title is now trimmed, stripped of control characters and truncated (empty becomes "Session"). A genuine database error still fails the start and stops the process.

Behaviour that changed in Core, as intended: an exited session stays in `list()` and can be replayed until closed; a consumer that goes away no longer ends the forwarder or marks the row `exited`; `terminate_workspace` now also forgets the sessions it stops; `SessionStarted` is published before `SessionExited` by a start gate rather than by ordering luck.
