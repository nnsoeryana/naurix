# ADR 0009 — Terminal data path and flow control
**Status:** Accepted (M3)

**Context:** Terminal output must reach xterm.js without entering React state, without unbounded buffering anywhere, and without adding keystroke latency.

**Decision:**
- Output travels Rust → Tauri `Channel` carrying **raw bytes** (`InvokeResponseBody::Raw`) → `xterm.write(Uint8Array)`. Exit is a separate JSON channel. React only holds a small lifecycle state.
- Rust coalesces already-queued chunks into batches ≤64 KiB **without a timer**, so an idle terminal echoes immediately and a flooding one sends large batches.
- Ack-based flow control: the webview acknowledges bytes after xterm has parsed them (`write` callback), batched (≥32 KiB or 16 ms). The forwarder pauses at 256 KiB unacknowledged. Together with the bounded reader queue (ADR 0008) a slow renderer slows the child process instead of growing memory.
- Input is sent as strings (`onData`) and bytes (`onBinary`); the IPC commands accept only a session id and data.
- **The frontend cannot choose what to execute.** M3 `terminal_spawn` starts only the user's login shell (`$SHELL`, validated, fallback `/bin/zsh`, with `-l`) in `$HOME`. Agents will use named presets (M5).
- A page (re)load terminates all sessions, because the JS state that owned them is gone; reattachment is not implemented (DATA-004).
- Renderer: xterm.js default DOM renderer for now; WebGL is a measured M10 option.

**Consequences:** `terminal_terminate` blocks up to ~5 s and runs on a blocking thread. Login shells with slow startup (a 16 s `zsh -l` was observed on the maintainer's machine) delay the first usable prompt; this is the user's configuration, not the app.
