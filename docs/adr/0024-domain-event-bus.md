# ADR 0024 — Domain event bus in Core (orchestration stage 2)
**Status:** Accepted (2026-11-23, maintainer). Implements stage 2 of ADR 0022. **No user-visible change.**

**Context:** ADR 0022 calls for domain events that several parties can consume (UI, later the run/workflow engine, audit). The audit found no event mechanism: the only push flows are per-session Tauri `Channel`s bound to one webview call. Core has no way to say "a session started" or "a session ended" to anyone except the one caller that holds the channel. `docs/TECHNICAL_DESIGN.md` §11 already names a `session://lifecycle` event.

**Decision**
1. Add `crates/core/src/events.rs`: an `Event` enum and an `EventBus` (publish / subscribe). No Tauri dependency.
2. **Metadata only.** Events carry ids, workspace id, session kind, adapter id and exit code/signal. They never carry terminal bytes, prompts, titles, environment values or file content. Terminal output keeps its own path (ADR 0009) and is not an event.
3. **Initial events, only what exists today:** `SessionStarted` and `SessionExited`. Nothing speculative (no task, run or approval events until those exist).
4. **Delivery is bounded and lossy by design.** Each subscriber gets a bounded queue. Publishing never blocks: if a subscriber's queue is full the event is dropped for that subscriber, and a disconnected subscriber is removed. A slow consumer therefore cannot stall a terminal. Consumers must treat events as notifications and re-read state (database or session list) when they need certainty. Nothing may rely on an event for correctness.
5. `SessionManager` owns a bus (`events()`), publishes `SessionStarted` after a successful start, and publishes `SessionExited` when the forwarder observes the process exit. `SessionManager::new` keeps its signature.
6. `src-tauri` forwards the bus to the webview as one Tauri event, `session://lifecycle`, with a camelCase payload. **No frontend code consumes it yet**; the stream exists so later stages and the UI can subscribe without changing Core again.
7. Event-driven refresh of existing UI state is **not** part of this stage.

**Consequences**
- Known limit, kept deliberately: `SessionExited` is published only when the forwarder sees the exit. If the consumer sink refuses output first, or the session is torn down by `shutdown`, no exit event is published. This mirrors the existing quirk that the session row is marked `exited` at that point. The session-ownership ADR (stage 4) resolves it together with reattach.
- Per ADR 0022 the status shown to users still comes only from process state or verified signals; these events add no agent-state claims.
- Implemented 2026-11-21: `crates/core/src/events.rs`, publishing in `session/mod.rs`, `src-tauri/src/events.rs`. `SessionStarted` is published before the forwarder thread starts so it always precedes `SessionExited`. `serde_json` was added to `src-tauri` as a **dev-dependency only** (already in the dependency tree via Core; one `Cargo.lock` line). The app was not launched interactively, so the Tauri emit itself is compile-checked but not observed at runtime.
- Tests: unit tests for fan-out, dropped subscriber, full queue not blocking; integration test that a real session publishes started then exited with the right code.
