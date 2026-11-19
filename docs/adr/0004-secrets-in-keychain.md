# ADR 0004 — Secrets live in macOS Keychain only
**Status:** Accepted
**Context:** MCP servers need tokens/API keys. The earlier Technical Design said "MVP can avoid secret storage", which is no longer true.
**Decision:** Secret values are stored only in the macOS Keychain (Rust, `security-framework`). SQLite stores a reference (service/account key), never the value. Secrets are never logged, never sent to the webview after save (UI shows "set / not set"), and are injected into the child process environment at launch. Syncing to an agent config that requires a literal value requires explicit user approval and a visible warning, because that writes the secret to a file outside our control.
**Consequences:** Unsigned dev builds trigger repeated Keychain prompts; document and sign dev builds.
