# ADR 0002 — Framework-independent `core` crate + thin Tauri shell
**Status:** Accepted
**Context:** PTY, supervisor, adapters, sync and persistence must be testable without a GUI, and domain code must not depend on Tauri (AGENTS.md rule).
**Decision:** Cargo workspace: `crates/core` (domain, pty, supervisor, adapters, mcp, skills, git, persistence; no Tauri dependency) and `src-tauri` (commands that validate input and delegate; event publishing; capabilities).
**Consequences:** Slightly more boilerplate; integration tests run with plain `cargo test`. A future non-Tauri frontend or native terminal surface can reuse `core`.
