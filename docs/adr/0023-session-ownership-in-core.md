# ADR 0023 — Session ownership moves into Core (orchestration stage 1)
**Status:** Proposed (maintainer to accept). Implements stage 1 of ADR 0022 only. **Behaviour-preserving refactor: no user-visible change.**

**Context:** ADR 0022 requires Core to own session start and lifecycle, with Tauri as a bridge. Today `src-tauri/src/terminal.rs` holds `Terminals`, `Entry`, `Launch` and `start_session`: it validates the workspace folder, spawns through `Supervisor`, inserts the `sessions` row, keeps the per-session flow-control state, runs the output-forwarding thread, and finishes the DB row on exit. That is domain logic in the Tauri layer, and it ties session start to Tauri `Channel`s.

**Decision**
1. Add `crates/core/src/session` with `SessionManager` (the former `Terminals`) and `SessionLaunch` (the former `Launch`). It owns the `Supervisor`, the per-session `Flow`, the workspace association, and the mapping from runtime `SessionId` to the database session row. It has no Tauri dependency.
2. `SessionManager::start` takes a **sink closure** (`FnMut(Outbound) -> bool + Send + 'static`) instead of Tauri channels. `src-tauri` supplies a sink that writes to its `Channel`s. The forwarding loop is still `pty::forward`, unchanged.
3. `write`, `resize`, `ack`, `has_foreground_job`, `terminate`, `terminate_workspace`, `shutdown` and `session_count` move with it. The termination grace period is set at construction (`SessionManager::new(db, grace)`), so every call keeps its current signature and timing.
4. `src-tauri/src/terminal.rs` keeps only the Tauri commands, the `Channel` adapter, and the debug/benchmark hooks. `SharedTerminals` stays as a type alias so other command files change minimally.
5. **Unchanged on purpose:** the wire session id (still the runtime `u64`, so the frontend is untouched), the shutdown-on-page-load policy, error message texts, thread names, batching and backpressure, and the database schema (no migration). A single id shared by runtime, database and UI, and sessions surviving reload, are separate stages (ADR 0022 stages 4 and later) and need their own ADR.

**Consequences**
- Session start and lifecycle become testable in Core without Tauri (new integration tests use real PTYs and an in-memory database).
- Known quirks are preserved rather than fixed here, to keep this change reviewable: if inserting the `sessions` row fails the session still runs without a row, and a failure to spawn the forwarding thread does not stop the already-started process. Both are listed for the later session-ownership ADR.
- Implemented 2026-11-21: `crates/core/src/session/mod.rs`, `crates/core/tests/session.rs` (4 tests), and a reduced `src-tauri/src/terminal.rs`. The app was **not** launched interactively; verification is `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace`, `tsc --noEmit` and `vitest`.
- Risk: a behavioural drift in lifecycle ordering. Mitigation: existing PTY tests stay green, plus new tests for start/exit/terminate/workspace-removal paths.
