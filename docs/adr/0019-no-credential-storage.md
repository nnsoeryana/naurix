# ADR 0019 — Naurix stores no credentials
**Status:** Accepted (partially implemented)

**Context:** The maintainer decided (2026-11-19) that Naurix should not keep any user credential. ADR 0004 (Keychain) and the AGENTS.md invariant "secrets only in macOS Keychain" assumed Naurix stores MCP secrets.

**Decision:** Values persisted in SQLite are checked by `credential::looks_like_credential` (known token prefixes, private key blocks, JWTs, passwords in URLs, long random tokens) and refused. The check is a best-effort guard; the UI also says not to enter credentials.

**Implemented:** custom agent environment values and arguments are checked (`agents::validate_custom`); tests in `credential.rs` and `tests/agents.rs`.

**Open:** MCP secrets still use the Keychain (ADR 0004). Removing that needs a decision on how credentialed MCP servers work without storage (environment-variable references vs. dropping them). Until then ADR 0004 stands for MCP only.
