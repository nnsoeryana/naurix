# Naurix — Product Requirements Document

**Version:** 1.2  
**Status:** Draft for implementation planning (see ADRs 0001–0007 for changes from 1.0; ADR 0022 adds the post-MVP orchestration scope in §5.9)  
**Platform:** macOS Intel (x86_64) and Apple Silicon (arm64)  
**License:** Apache-2.0

## 1. Summary

Naurix is a terminal-first desktop application that provides project workspaces, terminal sessions, and a unified interface for launching and monitoring AI coding CLI agents.

The MVP focuses on dependable terminal behavior, workspace/session organization, agent launch and basic lifecycle visibility, metadata persistence, and basic Git inspection.

## 2. Goals

- Deliver a reliable interactive terminal experience.
- Organize terminals and agent processes by project workspace.
- Launch Claude Code, Codex, Gemini CLI, OpenCode, and custom CLI commands.
- Provide useful, honest session status without requiring unsupported provider APIs.
- Persist workspace and session metadata locally.
- Provide basic Git status and diff inspection.
- Keep the architecture extensible for future adapters and platforms.
- After the MVP (ADR 0022, §5.9): run several agents concurrently as a provider-agnostic orchestration workspace, with every agent action under one user-controlled approval gate, opt-in project memory, and reusable workflows. Terminal use never depends on any of it.

## 3. Non-goals

- Autonomous orchestration: any agent action, delegation or workflow step that runs without user approval. Approved, user-controlled orchestration is in scope after the MVP (§5.9, ADR 0022).
- Automatic capture of prompts, transcripts or terminal output as "memory". Memory is explicit and opt-in (§5.9).
- A background daemon or supervisor process (ADR 0022).
- Built-in LLM/API provider or model-provider API key management (MCP server credentials are in scope; ADR 0004/0007).
- Public MCP or skills marketplace (a small static catalog and manual configuration only).
- Cloud accounts, sync, remote execution, or collaboration.
- Full code editor or IDE functionality.
- Guaranteed restoration of arbitrary processes after application termination.
- Persistent terminal transcript/history capture beyond normal shell history.
- Automatic approval of agent actions. (A user may grant a scoped approval through the approval gate, §5.9; Naurix never grants one itself.)

## 4. Personas

### P1 — AI-assisted developer
Uses one or more coding agents while developing in local repositories. Needs fast switching and visibility.

### P2 — Terminal power user
Uses shell tools, custom profiles, keyboard shortcuts, and split panes. Will reject the product if basic terminal behavior is unreliable.

### P3 — Adapter contributor
Wants to add support for another CLI agent without changing core application code.

## 5. MVP scope and requirements

Priority definitions:
- **P0:** required for MVP.
- **P1:** valuable, can follow the first usable release.
- **P2:** future.

### 5.1 Application shell and workspace

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| WS-001 | P0 | Create workspace from local folder | User can select a directory; workspace name and path are displayed |
| WS-002 | P0 | List and switch workspaces | Switching workspace updates its session list and active context |
| WS-003 | P0 | Rename and remove workspace from app | Removal does not delete project files |
| WS-004 | P0 | Persist workspace metadata | Workspaces remain listed after app restart |
| WS-005 | P1 | Workspace color/icon | User can customize workspace appearance |
| WS-006 | P1 | Recent workspaces | Recently opened workspaces are accessible |

### 5.2 Terminal

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| TERM-001 | P0 | Spawn interactive shell via PTY | Default shell starts in selected workspace directory |
| TERM-002 | P0 | Render terminal output | Supports ANSI colors, cursor movement, clear screen, and common escape sequences through xterm.js |
| TERM-003 | P0 | Forward keyboard input | Interactive shell input, control keys, and paste work |
| TERM-004 | P0 | Resize PTY | Terminal process receives updated rows and columns |
| TERM-005 | P0 | Create/close terminal sessions | Session lifecycle and UI state remain synchronized |
| TERM-006 | P0 | Multiple tabs | User can create, focus, rename, and close tabs |
| TERM-007 | P0 | Split panes | Support vertical and horizontal splits; each pane owns a session view |
| TERM-008 | P0 | Session title and process state | Display user-defined title and process running/exited state |
| TERM-009 | P0 | Terminal scrollback | Scrollback is available in memory for active session |
| TERM-010 | P1 | Search terminal buffer | User can search visible/retained buffer |
| TERM-011 | P1 | Profiles | Configure shell executable, args, environment additions, and starting directory |
| TERM-012 | P1 | SSH usage | Users can run their normal SSH command in shell; no custom SSH manager required |
| TERM-013 | P1 | Themes and font settings | Configure font family, size, cursor, and color theme |
| TERM-014 | P2 | Native libghostty backend | Prototype and compare before committing to production |

### 5.3 Agent management

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| AG-001 | P0 | Launch configured agent CLI | User can launch an agent in the selected workspace |
| AG-002 | P0 | Built-in launch presets | Provide presets for Claude Code, Codex, Gemini CLI, and OpenCode |
| AG-003 | P0 | Custom CLI agent | User can define executable, arguments, environment additions, and display name |
| AG-004 | P0 | Agent session list | Agent sessions appear separately from ordinary shell sessions |
| AG-005 | P0 | Basic lifecycle state | Display Starting, Running, Exited, Failed, or Unknown |
| AG-006 | P0 | Stop agent | User can terminate an agent session with confirmation when appropriate |
| AG-007 | P0 | Focus agent terminal | Selecting an agent opens/focuses its associated terminal view |
| AG-008 | P0 | Detect executable availability | Show whether configured CLI executable can be found |
| AG-009 | P1 | Waiting-for-input state | Only show when reliable signal exists; otherwise show Unknown/Running |
| AG-010 | P1 | Notifications | Notify when process exits or a reliable attention signal is received |
| AG-011 | P1 | Adapter capability display | Show structured protocol, status, and control capabilities per adapter |
| AG-012 | P2 | Agent-to-agent handoff | Explicit workflow with user approval (see ORCH-005, WF-001) |

**Important status rule:** process liveness is not equivalent to agent task state. The MVP must not claim an agent is waiting for approval or completed unless a reliable signal exists. Generic CLI sessions can show process state only.

### 5.4 Git inspector

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| GIT-001 | P0 | Detect Git repository | Show branch and repository state for workspace |
| GIT-002 | P0 | Show changed files | List modified, added, deleted, and untracked files |
| GIT-003 | P1 | View diff | Show textual diff for selected file |
| GIT-004 | P0 | Refresh Git state | Refresh after user action and relevant process events; do not assume every file write is observable |
| GIT-007 | P0 | Open repository in terminal | Opens a terminal session at the repository root |
| GIT-005 | P1 | Worktree list | Display worktrees associated with repository |
| GIT-006 | P2 | Create/manage worktrees | Create and remove worktrees with explicit user confirmation |

### 5.5 Persistence and recovery

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| DATA-001 | P0 | Store workspace metadata in SQLite | Workspace metadata survives restart |
| DATA-002 | P0 | Store session metadata | Session identity, type, title, workspace, timestamps, and last-known state are stored |
| DATA-003 | P0 | No persistent terminal transcript by default | Terminal output is not written to database or application logs. The only user text stored beyond metadata is opt-in memory notes (MEM-001 to MEM-007) |
| DATA-004 | P0 | Honest restart recovery | On restart of the application process, mark prior process sessions as interrupted/not reattachable. A webview reload or window close is not a restart and must not end sessions once SES-001/SES-002 are implemented (ADR 0022) |
| DATA-005 | P0 | Handle missing project directory | Show unavailable workspace and allow user to locate or remove it |
| DATA-006 | P1 | Configurable scrollback persistence | Only after explicit opt-in and security review |

**MVP recovery contract:** persist metadata, not process memory. The application must not imply that a process survived an application quit. On application quit, child processes are terminated gracefully (ADR 0022); this is tested and documented. There is no background daemon, so nothing outlives the Naurix process.

### 5.6 MCP integration manager

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| MCP-001 | P0 | Central MCP registry | Servers are stored in an app-managed registry independent of any agent's format |
| MCP-002 | P0 | Catalog + manual add | Static catalog (GitHub, PostgreSQL, Playwright, Filesystem, Context7, Sentry) and manual add of command, args, env, transport (stdio; remote HTTP/SSE where supported) |
| MCP-003 | P0 | Secrets in Keychain | Secret values never in SQLite, logs, or webview after save (ADR 0004) |
| MCP-004 | P0 | Enable/disable, edit, remove | Changes are reflected in synced agents only after an approved plan |
| MCP-005 | P0 | Approval before first run | A newly added server is never executed (including by Test Connection) until the user approves its command |
| MCP-006 | P0 | Validation and errors | Invalid config shows a specific, actionable error |
| MCP-007 | P0 | Test connection, list tools | Handshake result, error, and tool list shown when the server supports it |
| MCP-008 | P0 | Agent sync via adapters | Per-agent dry-run diff, backup, apply, rollback; user-owned entries are never overwritten (ADR 0003) |
| MCP-009 | P1 | Live status | Status shown only from real connection results |

### 5.7 Agent skills manager

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| SK-001 | P0 | Central skill registry | Skill content lives once in an app-managed store |
| SK-002 | P0 | Install from Git URL or local folder | Source and install location displayed before and after install |
| SK-003 | P0 | Show name, description, compatibility | Parsed from skill metadata; per-agent compatibility shown, unknown shown as unknown |
| SK-004 | P0 | Enable/disable per agent | Implemented by symlink via sync adapter; reversible |
| SK-005 | P0 | Update and uninstall | Uninstall requires confirmation and removes only what the app created |
| SK-006 | P0 | Untrusted content | Scripts in skills are never executed by the app; first install shows contents summary |
| SK-007 | P0 | No overwrite | Existing user files at a target path block the operation with a clear message |
| SK-008 | P1 | Category browsing | Frontend, backend, review, testing, debugging, architecture, documentation |

### 5.8 Settings and usability

Settings sections: General, Terminal, AI Agents, MCP Integrations, Agent Skills, Performance, Security.


| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| UX-007 | P0 | Settings sections | The seven sections above exist; common tasks need no manual config-file editing |
| UX-001 | P0 | Keyboard navigation | Core workspace, tab, pane, and command-palette actions are keyboard accessible |
| UX-002 | P0 | Command palette | Search and execute core application actions |
| UX-003 | P0 | Clear process feedback | Show starting, running, exited, and failed states |
| UX-004 | P0 | Permission prompts | Request filesystem access through user-initiated flows |
| UX-005 | P1 | Theme and font preferences | Settings survive restart |
| UX-006 | P1 | Notification preferences | User can disable notifications |

### 5.9 Orchestration workspace (post-MVP, ADR 0022)

Delivered in the stages listed in ADR 0022, each leaving the app runnable. Implemented (stages 1–9, ADR 0023–0031): SES-001–004, ORCH-001–007 (ORCH-007 Claude Code only), APR-001–003, GIT-006, WF-001 and MEM-001–008 (MEM-008 in the user-driven insert form only, ADR 0031 §6). Visual and real-agent checks are listed as not verified in each ADR's implementation log. None of it may be required to use the terminal.

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| SES-001 | P1 | Sessions survive UI reload | Reloading the webview leaves running sessions alive; the view reattaches. Any repaint buffer is in memory, bounded, never persisted |
| SES-002 | P1 | Sessions survive window close | Closing the window does not end sessions while the Naurix process is alive. Behaviour per platform is defined and tested before it is claimed |
| SES-003 | P1 | Graceful termination on quit | Quitting terminates agent and terminal processes gracefully (SIGHUP/SIGTERM/SIGKILL ladder); no orphans (tested) |
| SES-004 | P1 | One stable session identity | A single session id is used by runtime, database and UI; Core owns session start and lifecycle |
| ORCH-001 | P1 | Agent Runtime separate from Terminal Runtime | Terminal sessions work with the agent runtime absent or disabled; the agent runtime starts and stops processes only through the terminal runtime |
| ORCH-002 | P1 | Adapter trait with per-capability gating | Behaviour per agent lives in its adapter; a capability is usable only when its level is Verified (or maintainer-accepted for a reversible action). MCP verification does not imply headless, structured events, status or resume |
| ORCH-003 | P1 | Agents added through adapters | Codex, Gemini CLI, Antigravity CLI and custom CLIs start as PTY-only adapters and gain capabilities one at a time as each is verified |
| ORCH-004 | P1 | Concurrent runs | Several agent runs can be active at once, with a user-visible limit (Settings, default 4, range 1–16; a refused launch spends no approval); state shown only from process state or verified signals |
| ORCH-005 | P2 | Tasks, runs and dependencies | Task and run metadata with dependency edges; no output or prompts stored |
| ORCH-006 | P1 | Opt-in worktree isolation | A run may request a Git worktree; creation needs approval at "once" scope and is never silent (see GIT-006) |
| ORCH-007 | P1 | Verified status signals | Working, waiting for approval, waiting for input and turn finished are shown only from a signal the agent itself reported through a Verified adapter capability, opt-in per agent, never from output heuristics; hooks never read prompts, answers or tool input; nothing is written to the agent's own configuration. Claude Code only so far (ADR 0029) |
| APR-001 | P1 | Central approval gate | Launching an agent, automated input, run or workflow step, worktree creation and agent-config writes all pass one gate that records and can revoke decisions |
| APR-002 | P1 | Scoped approvals | Scopes: once, workspace and app (ADR 0027), and run, created only by Core for an approved workflow run and ended with it (ADR 0030); a session scope is not used. Bound to a fingerprint of the reviewed target; a changed target needs a new approval. Git mutations and agent-config writes are "once" only. No "approve everything" scope |
| APR-003 | P1 | Approval visibility | Approvals are listed in Settings (Security), revocable, cleared with the workspace; stored as metadata only |
| WF-001 | P2 | Reusable workflows | A workflow is a saved set of tasks and dependencies, runnable many times; every step goes through the approval gate |
| MEM-001 | P2 | Opt-in per workspace | Memory is off until the user enables it for that workspace |
| MEM-002 | P2 | Explicit content only | Only notes the user writes or approves. Prompts, transcripts and terminal output are never stored automatically |
| MEM-003 | P2 | No secrets | Suspected credentials are rejected on save; secrets stay in the OS credential store |
| MEM-004 | P2 | SQLite with FTS5 | Memory is stored in SQLite and searchable with FTS5; FTS5 availability in the shipped build is verified first; deleting an entry also deletes its index rows |
| MEM-005 | P2 | View, edit, delete | Per entry and per workspace; removing a workspace removes its memory |
| MEM-006 | P2 | Export | Markdown and JSON, to a location the user chooses; a warning if that location is inside a Git repository |
| MEM-007 | P2 | Never committed automatically | Naurix never writes memory or exports into a repository and never runs `git add`/`git commit` for them |
| MEM-008 | P2 | Approved delivery to agents | Memory reaches an agent only through an approved, reversible mechanism (dry-run, backup, rollback); never injected silently |

### 5.10 Security Suite (ADR 0032)

Red, Blue and Purple are roles delivered as skills and custom agents; Naurix itself only reads and compares security reports the user opens. Optional: none of it is needed to use terminals or agents. Implemented so far: SEC-001 to SEC-003 in Core only (no view yet).

| ID | Priority | Requirement | Acceptance criteria |
|---|---|---|---|
| SEC-001 | P2 | Read SARIF reports as untrusted input | Only SARIF 2.1.0; at most 20 MiB and 5,000 results; text cleaned and length-limited; paths made relative to the workspace or dropped, checked lexically; errors never quote report content |
| SEC-002 | P2 | Findings are never stored | No finding, message or report path is written to the database, logs or events; a report lives only while the view shows it |
| SEC-003 | P2 | Regression comparison | Two reports opened together are compared by fingerprint into new, still present and no longer reported; "no longer reported" is never called fixed |
| SEC-004 | P2 | Report opened by the user | The backend opens the file dialog; the webview never names a path; nothing is read in the background |
| SEC-005 | P2 | Scanner exit codes | A custom agent may list the exit codes that mean success; the list is part of its launch fingerprint |
| SEC-006 | P2 | Offensive targets are approved | Network-facing security actions need a once/run approval whose fingerprint covers the exact targets; default policy is the workspace folder and loopback |

## 6. UX and information architecture

(Section numbering of 5.x was extended: MCP is 5.6, Skills 5.7, Settings 5.8, Orchestration 5.9.)

### Main window
- **Sidebar:** workspace list and session/agent list.
- **Main area:** tab strip and terminal pane layout.
- **Inspector:** collapsible right panel for session details and Git.
- **Command palette:** overlay triggered by keyboard shortcut.
- **Status bar:** active workspace, shell/session status, Git branch.

### Empty states
- No workspace: explain how to open a local project.
- Workspace without sessions: offer New Terminal and Launch Agent.
- Agent executable missing: show installation/configuration guidance; do not install software automatically.
- Non-Git folder: explain that Git inspector is unavailable.
- Restored metadata for an interrupted process: label it as previous session metadata, not a live session.

### Keyboard-first requirements
Provide configurable shortcuts. Initial defaults should be reviewed against macOS conventions:
- New terminal: Cmd+T.
- Command palette: Cmd+K.
- Close tab/session: Cmd+W.
- Split pane shortcuts must avoid conflicts with common macOS and shell shortcuts.
- Do not hardcode shortcuts in multiple UI components; centralize shortcut definitions.

## 7. Agent adapter contract

An adapter describes how to launch and optionally observe/control an agent CLI.

Minimum adapter metadata:
- Stable adapter ID and display name.
- Executable candidates.
- Default arguments.
- Capability declaration.
- Version/detection strategy.
- Launch configuration schema.
- Optional structured event parser.
- Optional status provider.

Capabilities should be explicit, for example:
- `pty_launch`
- `structured_events`
- `reliable_status`
- `send_message`
- `graceful_stop`
- `resume_session`

A capability must not be advertised unless implemented and tested. PTY launch is the universal fallback.

Verification is per capability (ADR 0014, ADR 0022). A Verified MCP capability (today Claude Code and OpenCode) does not make headless execution, structured events, status signals or resume verified; each needs its own evidence and agent version. Optional behaviour is reached only through the adapter, so adding an agent should not require edits outside its adapter.

## 8. Functional flows

### Flow A — Create workspace
1. User selects Add Workspace.
2. Native folder picker opens.
3. App validates the selected path.
4. App stores workspace metadata.
5. Workspace becomes active.
6. User sees empty-state actions.

### Flow B — New terminal
1. User selects New Terminal.
2. App resolves configured shell.
3. Rust process service creates PTY in workspace directory.
4. Frontend attaches xterm.js to session stream.
5. Resize and input events are forwarded.
6. Exit event updates session state.

### Flow C — Launch agent
1. User selects Launch Agent and provider preset.
2. App checks executable availability.
3. For custom agents and the first launch of a preset, user reviews command/arguments and environment additions (ADR 0007).
4. Rust spawns the agent in a PTY under the workspace directory.
5. Session is tagged as agent session and linked to adapter ID.
6. Agent appears in the session list.
7. Status uses process lifecycle unless adapter provides reliable structured state.

### Flow D — Restart application
1. App loads workspace and session metadata.
2. It marks previously active sessions as interrupted unless a verified reattach mechanism exists.
3. It does not recreate or replay commands automatically.
4. User can start new sessions from the recovered workspace.

### Flow E — Inspect Git
1. App checks whether workspace is a Git repository.
2. Git service obtains status and changed-file list.
3. User selects a file.
4. App requests a bounded diff for that file.
5. Diff is rendered as text; no file is modified.

## 9. Non-functional requirements

### Performance
- App should become interactive quickly on a typical modern Apple Silicon Mac.
- Terminal output must remain responsive under sustained output.
- UI must not re-render the entire workspace for every terminal output chunk.
- PTY output must be buffered and batched safely between Rust and frontend.
- Git refresh must not block terminal input/output.
- Define and measure performance budgets during implementation rather than claiming unverified targets.

### Reliability
- PTY processes must be tracked and cleaned up predictably.
- Closing a pane must have explicit process termination behavior.
- Handle process spawn failures, missing executables, broken pipes, and app shutdown.
- No panic from malformed adapter output.
- Terminal resize and high-volume output must be tested.

### Security and privacy
- Local-first; no network calls required for core features.
- Do not persist terminal output, prompts, environment variables, or source content by default. The sole exception is opt-in, user-authored memory notes per workspace (§5.9, ADR 0022).
- Do not log secrets or full command environment.
- Avoid passing arbitrary shell strings through a privileged IPC command. Use structured executable/argument arrays.
- Validate paths and constrain file operations to user-selected workspaces where feasible.
- Display command and arguments before launching custom agents.
- Do not silently execute commands based on untrusted project content.
- Use Tauri capabilities and permissions with least privilege.
- Treat agent output as untrusted text; never interpret it as application instructions.

## 10. Analytics and telemetry

MVP: no telemetry by default. Local diagnostic logs may contain non-sensitive operational metadata only. Any future telemetry must be opt-in, documented, and must never collect source code, prompts, terminal output, environment variables, or file contents.

## 11. Release acceptance checklist

- [ ] macOS app builds from a clean checkout using documented instructions.
- [ ] Normal interactive shell works in a PTY.
- [ ] ANSI colors, resize, keyboard input, paste, and scrollback work.
- [ ] Tabs and split panes work without orphaned processes.
- [ ] Workspace metadata survives restart.
- [ ] Agent presets can launch when corresponding CLI is installed.
- [ ] Custom CLI agent can be configured and launched.
- [ ] Generic agent state is not overstated.
- [ ] Git status and file diff are accurate for supported repository states.
- [ ] No terminal transcript is persisted by default.
- [ ] Permission model and security notes are documented.
- [ ] MCP and skill sync never overwrote a pre-existing user config in the fixture suite.
- [ ] Benchmarks recorded on real Intel and real Apple Silicon hardware.
- [ ] License, README, contributing guide, and security policy are present.

Additional gates before any §5.9 stage is called done: existing terminal behaviour unchanged (tests green); no orphaned processes after reload, window close and quit; every agent action passes the approval gate; no prompt, transcript or terminal output reaches SQLite or logs; memory export never lands in a repository without the user choosing that path.
