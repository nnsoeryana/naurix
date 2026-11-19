# ADR 0008 — PTY crate and process lifecycle
**Status:** Accepted (M2). Resolves the "which PTY crate" open question; first results of Spike D.

**Context:** Need PTY spawn/IO/resize/terminate on macOS x86_64 and arm64 with predictable cleanup.

**Decision:**
- Use `portable-pty` 0.9.0 (wezterm; MIT; ~30M downloads). Risk: last release Feb 2025, so maintenance is slower than ideal; PTY access stays behind `crates/core::pty`, so it can be swapped for direct `libc`/`nix` code if it stalls.
- The child is a session leader (portable-pty calls `setsid`), so its pgid equals its pid.
- One reader thread per session is the **only** sender on a bounded queue (64 × ≤32 KiB). A slow consumer blocks the reader, the kernel PTY buffer fills, and the child blocks: memory stays bounded (tested deterministically: a child writing 20 MB stays blocked while nobody reads, and every byte arrives once drained). `Exited` is always the last event.
- The reader `poll(2)`s a dup'd master fd so it notices child exit even if a grandchild keeps the slave open.
- `terminate(grace)`: SIGHUP → SIGTERM → SIGKILL, spaced by `grace/3`, sent to the session pgid **and** the PTY's current foreground pgid (interactive shells run jobs in their own groups), followed by a SIGKILL sweep of both groups. SIGHUP comes first because interactive shells ignore SIGTERM.
- `Supervisor` terminates all sessions on `Drop` (1 s grace) and the Tauri shell calls `shutdown_all(3 s)` on `RunEvent::Exit`. Natural exit of a shell does **not** sweep leftover background jobs (nohup semantics are preserved).
- Input is size-limited (1 MiB per write) and rejected, never truncated; PTY sizes are 1..=1000.

**Verified (x86_64, macOS 14.6, 14 integration tests, repeated runs):** exit codes, input round-trip, resize visible to child via `stty size`, cwd and env additions, error cases, backpressure bound, SIGKILL escalation for a process ignoring HUP/TERM, background-child cleanup, interactive `zsh` foreground and background jobs cleaned up, shutdown of 4 sessions, cleanup on supervisor drop.

**Not yet verified / known limits:**
- arm64: tests have not been run on Apple Silicon.
- A process that deliberately detaches (`setsid`, double-fork daemon) can escape.
- Shutdown on SIGTERM/SIGKILL of the app itself, and on crash, is untested; a hard-killed app leaves children to the kernel's PTY hangup behavior. To be covered when sessions can be created from the UI (M3/M4).
- `terminate` blocks up to ~`grace + 2 s`; Tauri commands must call it from a blocking context.
