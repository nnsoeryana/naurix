# Coding Agent Instructions — Naurix

You are contributing to Naurix, an open-source, terminal-first desktop application for managing normal shell sessions and AI coding CLI agents.

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
- Metadata-first persistence: do not persist terminal input/output, prompts, environment values, or source content by default.
- User control: do not silently execute commands, approve agent actions, or mutate Git state.
- Hybrid agent support: PTY CLI launch is the fallback; structured capabilities are optional and must be verified.
- Secrets only in macOS Keychain; never in SQLite, logs, or the webview after save.
- Never overwrite user-owned agent config or files; MCP/skill sync is dry-run, approved, backed up, and reversible. Never execute skill scripts. Never run an unapproved MCP server.
- Do not claim Intel/Apple Silicon support or performance numbers without a recorded run on that hardware.
- Do not assume an agent's config format or skill directory; verify against current official docs.
- Do not claim an agent is waiting for approval/completed unless a reliable adapter signal supports it.

## Engineering rules

- Keep process execution and PTY management in Rust.
- Keep domain logic independent from React and Tauri command handlers.
- Tauri commands validate input and delegate to services.
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

Do not implement future orchestration, cloud execution, team collaboration, or a full IDE unless explicitly requested. Do not introduce speculative abstractions that are not needed by the current milestone.

## Definition of done

- Acceptance criteria are met.
- Tests cover normal behavior and important failure paths.
- No secrets or terminal content are logged or persisted.
- Errors are actionable and do not crash the application.
- Documentation is updated where behavior or architecture changes.
