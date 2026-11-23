# Architecture Decision Records

Each ADR is immutable once Accepted; change a decision by adding a new ADR that supersedes it.
Format: Context → Decision → Consequences. Status: Proposed | Accepted | Superseded.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-platform-scope.md) | Platform scope: macOS x86_64 + arm64 | Accepted (Windows/Linux clause superseded by 0021) |
| [0002](0002-core-crate-split.md) | Framework-independent `core` crate + thin Tauri shell | Accepted |
| [0003](0003-central-registry-with-sync-adapters.md) | Central registry + per-agent sync adapters (MCP, Skills) | Accepted |
| [0004](0004-secrets-in-keychain.md) | Secrets live in macOS Keychain only | Accepted |
| [0005](0005-git-via-cli.md) | Git integration shells out to `git` | Accepted |
| [0006](0006-performance-budgets-are-hypotheses.md) | Performance budgets are hypotheses until benchmarked | Accepted |
| [0007](0007-scope-clarifications.md) | Scope clarifications vs. the initial vision docs | Accepted |
| [0008](0008-pty-crate-and-process-lifecycle.md) | PTY crate and process lifecycle | Accepted |
| [0009](0009-terminal-data-path.md) | Terminal data path and flow control | Accepted |
| [0010](0010-workspaces-tabs-panes.md) | Workspaces, tabs, panes and persistence | Accepted |
| [0011](0011-agent-launch-generic-pty.md) | Agent launch via the generic PTY adapter | Accepted |
| [0012](0012-terminal-fonts-and-theme.md) | Terminal fonts and default theme | Accepted |
| [0013](0013-naurix-rename-and-brand.md) | Rename to Naurix and adopt the Aurora brand | Accepted |
| [0014](0014-adapter-verification-levels.md) | Adapter facts carry a verification level | Accepted |
| [0015](0015-mcp-manager.md) | MCP manager: registry, secrets, approval, test, sync | Accepted |
| [0016](0016-skills-manager.md) | Skills manager: central store, symlink links, untrusted content | Accepted |
| [0017](0017-git-status.md) | Read-only Git status, hardened against hostile repositories | Accepted |
| [0018](0018-settings-and-performance-budgets.md) | Settings model and performance budgets | Accepted |
| [0019](0019-no-credential-storage.md) | Naurix stores no credentials | Accepted (partially implemented) |
| [0020](0020-theme-system.md) | Theme system | Accepted |
| [0021](0021-windows-linux-readiness.md) | Windows and Linux readiness | Accepted (implemented, unvalidated) |
| [0022](0022-orchestration-workspace-scope.md) | Orchestration workspace scope, session lifecycle and shared project memory | Accepted |
| [0023](0023-session-ownership-in-core.md) | Session ownership moves into Core (orchestration stage 1) | Proposed |
| [0024](0024-domain-event-bus.md) | Domain event bus in Core (orchestration stage 2) | Proposed |
| [0025](0025-agent-adapter-trait.md) | `AgentAdapter` trait for per-agent behaviour (orchestration stage 3) | Proposed |
| [0026](0026-sessions-outlive-the-view.md) | Sessions outlive the view: detach, reattach and window close (orchestration stage 4) | Proposed (implemented; manual macOS checks pending) |
| [0027](0027-approval-gate.md) | Central approval gate (orchestration stage 5) | Proposed (implemented; visual and real-agent checks pending) |
| [0028](0028-worktree-isolation.md) | Opt-in Git worktree isolation for agents (orchestration stage 6) | Proposed (implemented; visual and real-agent checks pending) |
