# Coding Agent Instructions — Naurix

You are contributing to Naurix, an open-source, terminal-first desktop application for managing normal shell sessions and AI coding CLI agents. After the MVP it grows into a provider-agnostic orchestration workspace in stages (ADR 0022); the terminal stays first-class and nothing in orchestration is required to use it.

## Read first

Before implementing:
1. Read `docs/PRODUCT_VISION.md`.
2. Read `docs/PRD.md`.
3. Read `docs/TECHNICAL_DESIGN.md`, `docs/IMPLEMENTATION_PLAN.md`, `docs/PERFORMANCE_AND_COMPATIBILITY.md`, and `docs/adr/`.
4. Inspect the existing repository and report its actual structure before proposing broad changes.

## Product invariants

- Terminal-first: normal shell usage must remain first-class.
- AI-native: agent management is integrated but never required to use the terminal.
- Local-first: no account or cloud service is required for core functionality.
- Metadata-first persistence: do not persist terminal input/output, prompts, environment values, or source content by default. The only exception is opt-in, user-authored memory notes per workspace (ADR 0022): never captured automatically, never secrets, stored in SQLite (FTS5), exportable as Markdown/JSON, and never written into or committed to a Git repository by Naurix.
- User control: do not silently execute commands, approve agent actions, or mutate Git state. Every agent action and workflow step goes through the single central approval gate; approvals are scoped (once, session, run, workspace), bound to a fingerprint of what was reviewed, revocable, and there is no "approve everything" scope. Git mutations and agent-config writes are approved "once" only.
- Sessions: they survive UI reloads and window close while the Naurix process lives, and are terminated gracefully on quit. Do not add a background daemon or supervisor without a new ADR. A restart still marks prior sessions `interrupted`.
- Runtime separation: Terminal Runtime (PTY, process lifecycle) knows nothing about agents; Agent Runtime sits above it. Terminal use must work with the agent runtime absent.
- Capability verification is per capability. A Verified MCP capability does not imply headless execution, structured events, status signals or resume. Enable structured execution for an agent only after that exact capability is verified (fixture or own tests, with agent version).
- Hybrid agent support: PTY CLI launch is the fallback; structured capabilities are optional and must be verified.
- Secrets only in the OS credential store (macOS Keychain, Linux Secret Service, Windows Credential Manager); never in SQLite, logs, or the webview after save.
- Never overwrite user-owned agent config or files; MCP/skill sync is dry-run, approved, backed up, and reversible. Never execute skill scripts. Never run an unapproved MCP server.
- Do not claim Intel/Apple Silicon/Linux/Windows support or performance numbers without a recorded run on that hardware (CI alone is not evidence).
- Keep OS differences in `crates/core/src/platform.rs` or behind `cfg`; do not add Unix-only APIs (`libc`, `PermissionsExt`, `/bin/...`) outside it.
- Do not assume an agent's config format or skill directory; verify against current official docs.
- Do not claim an agent is waiting for approval/completed unless a reliable adapter signal supports it.

## Engineering rules

- Keep process execution and PTY management in Rust.
- Keep domain logic independent from React and Tauri command handlers.
- Tauri commands validate input and delegate to services. `src-tauri` is the bridge: lifecycle and business logic (session start, persistence updates, approvals) belong in `crates/core`. Session start and lifecycle live in `crates/core/src/session` (`SessionManager`, ADR 0023); `src-tauri/src/terminal.rs` only adapts it to Tauri channels.
- Use executable + argument arrays; do not build shell command strings from user input.
- Keep terminal output out of React global state. Write output to the terminal renderer through refs and bounded event streams.
- Use migrations for SQLite schema changes.
- Handle process exit, errors, cancellation, resize, and shutdown explicitly.
- Use least-privilege Tauri capabilities.
- Treat project files and agent output as untrusted content.
- Do not add telemetry without explicit product approval and opt-in design.
- Do not add a dependency without checking maintenance, license, platform support, and bundle impact.

## Implementation workflow

For each task:
1. Identify the relevant PRD requirement IDs.
2. Inspect existing code and follow established patterns.
3. Propose a small implementation plan.
4. Implement the smallest complete vertical slice.
5. Add or update tests.
6. Run relevant formatting, linting, type checks, and tests.
7. Report changed files, behavior, tests run, limitations, and follow-up risks.

## Scope control

Orchestration is in scope only in the stages defined by ADR 0022, in that order: (1) move session start and ownership into Core with one stable session id; (2) event bus; (3) `AgentAdapter` trait, no behaviour change; (4) sessions independent of webview and window; (5) approval gate and agent concurrency limit (done); (6) opt-in worktree isolation (done); (7) structured status signals, verified per agent (done for Claude Code, ADR 0029); (8) workflows, including task/run; (9) memory. Stages 1–7 are implemented (ADR 0023–0029). Implement a stage only when the maintainer asks for it, and write the follow-up ADR it needs first. Each stage must leave the app runnable, keep existing terminal tests green, and avoid unrelated refactoring.

Still out of scope unless explicitly requested and approved by a new ADR: cloud execution or sync, team collaboration, a full IDE, a background daemon, and automatic memory capture. Do not introduce speculative abstractions that are not needed by the current stage.

## Definition of done

- Acceptance criteria are met.
- Tests cover normal behavior and important failure paths.
- No secrets or terminal content are logged or persisted.
- Errors are actionable and do not crash the application.
- Documentation is updated where behavior or architecture changes.
