# ADR 0005 — Git integration shells out to `git`
**Status:** Accepted
**Context:** Open question in the Technical Design: Git executable vs Rust library.
**Decision:** Invoke the installed `git` with argument arrays (`status --porcelain=v2 -z --branch`, bounded `diff`). No libgit2/gix dependency. Read-only in the MVP. Missing `git` yields a clear empty state.
**Consequences:** Smaller binary, matches user's Git config/behavior; depends on a system `git` (Command Line Tools provide one on macOS). Output parsing is covered by fixture tests.

**Addendum (M9):** the hardening, trust gate and refresh policy that make this safe on repositories we do not control are in ADR 0017.
