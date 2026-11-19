# Naurix — Technical Design

**Version:** 1.1  
**Status:** Proposed architecture for MVP (decisions in docs/adr)  
**Target:** macOS x86_64 + arm64; portable core  
**Stack:** Tauri 2, Rust, React, TypeScript, xterm.js, SQLite

## 1. Architecture goals

- Separate terminal process management from agent-specific behavior.
- Keep process execution in Rust, not the webview.
- Keep UI state in React and durable metadata in SQLite.
- Make provider integrations replaceable adapters.
- Make the terminal renderer replaceable so libghostty can be evaluated.
- Avoid storing terminal content by default.
- Keep core domain types independent of Tauri and React where practical.

## 2. High-level architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       React + TypeScript                     │
│                                                             │
│  Workspace UI   Terminal UI   Agent UI   Git Inspector       │
│  Command Palette   Settings   Session Navigation             │
└──────────────────────────────┬──────────────────────────────┘
                               │ Tauri commands / events
┌──────────────────────────────▼──────────────────────────────┐
│                         Tauri 2 Shell                        │
│              IPC boundary / capabilities / lifecycle         │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│                           Rust Core                          │
│                                                             │
│ Workspace Service   Session Service   Settings Service       │
│ PTY Service         Agent Runtime      Git Service            │
│ Event Publisher     Persistence        Process Supervisor     │
└───────────────┬───────────────────────┬─────────────────────┘
                │                       │
       ┌────────▼────────┐     ┌────────▼────────────────────┐
       │ PTY / Shell     │     │ Agent Adapter Registry      │
       │ zsh/bash/fish   │     │ Claude / Codex / Gemini      │
       │                 │     │ OpenCode / Custom CLI       │
       └─────────────────┘     └─────────────────────────────┘

                       SQLite metadata store
```

## 3. Technology decisions

| Concern | Choice | Notes |
|---|---|---|
| Desktop shell | Tauri 2 | Native window, IPC, packaging |
| Backend | Rust | Process and filesystem boundary |
| Frontend | React + TypeScript | UI and terminal view integration |
| Terminal renderer | xterm.js | MVP renderer; isolate behind TerminalView interface |
| PTY | portable-pty or equivalent reviewed crate | Validate macOS support and maintenance before adoption |
| Database | SQLite (`rusqlite`) | Metadata only |
| Secrets | macOS Keychain (`security-framework`) | ADR 0004 |
| Git | `git` CLI | ADR 0005 |
| Frontend state | Zustand | UI state; do not duplicate database as a second source of truth |
| Styling | Tailwind CSS v4 | ADR 0007 |
| Schema validation | Zod in frontend; Rust validation in backend | IPC inputs must be validated on both sides |
| Tests | Rust tests, Vitest, Playwright | Add PTY integration tests |
| Package manager | pnpm | Frontend workspace if monorepo is used |

### Terminal engine strategy: both, through an abstraction

Do not implement two terminal engines in the first MVP. Implement a renderer boundary and run a technical spike comparing xterm.js and libghostty.

Proposed frontend interface:

```ts
export interface TerminalViewHandle {
  write(data: Uint8Array): void;
  resize(cols: number, rows: number): void;
  focus(): void;
  dispose(): void;
}

export interface TerminalViewProps {
  sessionId: string;
  onInput(data: Uint8Array): void;
  onResize(cols: number, rows: number): void;
}
```

The interface is conceptual; adapt it to the chosen renderer's actual API. A future libghostty integration may require a native rendering surface and a different data path.

Decision gate:
- Keep xterm.js if it meets compatibility, performance, accessibility, and packaging requirements.
- Adopt libghostty only after a spike proves integration feasibility, license compatibility, maintainability, and measurable user benefit.
- Do not create a dual-engine production setting until both implementations are stable and tested.

### Workspace layout (ADR 0002)

```text
crates/core/      # no Tauri dependency: domain, pty, supervisor, adapters, mcp, skills, git, persistence
src-tauri/        # command handlers (validate, delegate), events, capabilities
src/              # React UI
bench/            # RSS/CPU benchmark harness
```
The Rust module list in section 5 describes the modules; they live in `crates/core` except `commands/` and `events/`, which live in `src-tauri`.

## 4. Core domain model

Keep domain models separate from UI components and raw database records.

### Workspace

```ts
type Workspace = {
  id: string;
  name: string;
  rootPath: string;
  createdAt: string;
  updatedAt: string;
  lastOpenedAt?: string;
};
```

### Session

```ts
type SessionKind = "terminal" | "agent";

type SessionLifecycle =
  | "starting"
  | "running"
  | "exited"
  | "failed"
  | "interrupted"
  | "unknown";

type Session = {
  id: string;
  workspaceId: string;
  kind: SessionKind;
  title: string;
  lifecycle: SessionLifecycle;
  adapterId?: string;
  processId?: number;
  startedAt?: string;
  endedAt?: string;
  createdAt: string;
  updatedAt: string;
};
```

Do not persist a PID as proof that a process is still owned by the app (the `sessions` table has no PID column; ADR 0010). PIDs can be reused. If process reattachment is added later, use stronger process identity and ownership checks.

### Agent adapter descriptor

```ts
type AgentCapability =
  | "pty_launch"
  | "structured_events"
  | "reliable_status"
  | "send_message"
  | "graceful_stop"
  | "resume_session";

type AgentAdapterDescriptor = {
  id: string;
  displayName: string;
  executableCandidates: string[];
  defaultArgs: string[];
  capabilities: AgentCapability[];
};
```

### Agent state model

Separate process lifecycle from agent task state:

```ts
type ProcessState =
  | "starting"
  | "running"
  | "exited"
  | "failed"
  | "unknown";

type AgentTaskState =
  | "working"
  | "waiting_for_input"
  | "completed"
  | "needs_approval"
  | "unknown";
```

`AgentTaskState` must be supplied only by a reliable adapter signal. Generic PTY output must not be heuristically presented as authoritative task state.

## 5. Rust module boundaries

Suggested modules:

```text
src-tauri/src/
├── main.rs
├── app_state.rs
├── commands/
│   ├── mod.rs
│   ├── workspace.rs
│   ├── terminal.rs
│   ├── session.rs
│   ├── agent.rs
│   ├── git.rs
│   └── settings.rs
├── domain/
│   ├── workspace.rs
│   ├── session.rs
│   └── agent.rs
├── services/
│   ├── workspace_service.rs
│   ├── session_service.rs
│   ├── terminal_service.rs
│   ├── agent_runtime.rs
│   ├── git_service.rs
│   └── process_supervisor.rs
├── adapters/
│   ├── mod.rs
│   ├── registry.rs
│   ├── claude.rs
│   ├── codex.rs
│   ├── gemini.rs
│   ├── opencode.rs
│   └── custom.rs
├── persistence/
│   ├── mod.rs
│   ├── database.rs
│   ├── migrations/
│   └── repositories/
└── events/
    ├── mod.rs
    └── publisher.rs
```

The exact folder structure can evolve, but maintain these dependency rules:
- Domain types must not depend on Tauri.
- Adapters depend on agent/process abstractions, not UI.
- Commands validate IPC input and call services; they should not contain business logic.
- Persistence repositories do not own process lifecycle.
- UI does not spawn processes directly.

## 6. PTY and terminal data flow

### Spawn
1. Frontend invokes `terminal_spawn` with a structured request.
2. Rust validates workspace and shell configuration.
3. Rust creates a PTY pair with requested initial rows/columns.
4. Rust spawns the shell with explicit executable, argument list, working directory, and environment additions.
5. Rust stores an in-memory process handle keyed by session ID.
6. Rust emits session lifecycle event.
7. Frontend attaches xterm.js to the session.

### Input
1. xterm.js receives keyboard input.
2. Frontend sends input bytes through a bounded IPC command or channel.
3. Rust writes bytes to the PTY master.
4. Backpressure and write errors are handled; input must not be silently dropped.

### Output
1. Rust reads PTY output asynchronously.
2. Output is emitted in bounded chunks through Tauri events/channels.
3. Frontend writes bytes into xterm.js.
4. UI updates must be batched to avoid React re-rendering per chunk.
5. Output is not persisted by default.

### Resize
1. xterm.js reports columns and rows.
2. Frontend sends resize request.
3. Rust updates PTY size.
4. Process receives terminal resize signal where supported.

### Termination
- Distinguish graceful close, process exit, and forced termination.
- Closing a session should prompt or follow a clearly configured policy if a process is active.
- Ensure child process handling is tested. Process groups and signal semantics must be explicit on macOS.
- On app shutdown, define and test whether children are terminated. Do not rely on implicit OS behavior.

## 7. Agent runtime and hybrid execution

### Universal fallback: PTY CLI launch

Every agent can be launched as a command in a PTY:
- executable path
- argument array
- working directory
- explicit environment additions
- session ID
- adapter ID

Avoid building a shell command string from user data. Prefer `Command` with executable and argument arrays to prevent quoting and injection errors.

### Structured integrations

Some agents may offer machine-readable output, SDKs, or session control. These are optional adapter capabilities, not assumptions shared by every provider.

Adapter lifecycle:
1. `detect`: resolve executable and optionally inspect version.
2. `build_launch_spec`: return executable, args, environment additions, and working directory.
3. `parse_event` (optional): convert documented structured events to normalized events.
4. `get_capabilities`: declare supported features.
5. `terminate`: use graceful stop where supported, otherwise process supervisor.

Do not reverse-engineer undocumented private protocols as a foundation for core behavior. Provider-specific features should be version-tested and isolated.

### Custom CLI adapter

Allow users to configure:
- Display name.
- Executable path or command lookup name.
- Arguments as a list of fields.
- Environment additions (never display secret values after save unless user explicitly chooses).
- Working directory policy.
- Optional icon/color.

Do not support arbitrary pre-launch shell scripts in MVP unless there is a clear security design.

## 8. Process supervision

Maintain an in-memory registry:

```text
Session ID -> Process Handle + PTY Reader/Writer + Lifecycle
```

Requirements:
- Unique session IDs.
- Track child process exit codes.
- Handle read/write errors.
- Avoid orphaned reader tasks.
- Bounded output queues.
- Backpressure strategy.
- Explicit shutdown and close semantics.
- No PID-only reattachment.

For macOS, test process groups, signal propagation, and child process cleanup. Cross-platform implementations may require platform-specific modules behind a common interface.

## 9. Persistence design

SQLite stores metadata only.

Proposed tables:

```sql
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  root_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_opened_at TEXT
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  adapter_id TEXT,
  lifecycle TEXT NOT NULL,
  started_at TEXT,
  ended_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Implementation notes:
- Use migrations from the first release.
- Validate enum values when reading old data.
- On app startup, mark previously active sessions `interrupted` unless verified reattachment exists.
- Never persist terminal output, shell input, environment values, prompts, or source content by default.
- Consider storing workspace paths as local absolute paths; handle missing or moved folders.
- Avoid storing sensitive custom-agent environment values in plaintext. Prefer OS keychain integration if secret persistence becomes necessary; MVP can avoid secret storage.

## 9a. MCP integration architecture (ADR 0003, 0004)

```text
UI ──► mcp commands ──► McpService ──► Registry (SQLite metadata)
                              │            └─ secret refs ──► Keychain
                              ├─► Connection tester (spawns approved server, MCP handshake, tools/list)
                              └─► SyncAdapter[agent]: detect / read_existing / plan / apply / rollback
```
Tables (metadata only):
```sql
CREATE TABLE mcp_servers (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, transport TEXT NOT NULL, -- stdio | http | sse
  command TEXT, args_json TEXT NOT NULL, url TEXT,
  env_json TEXT NOT NULL,        -- non-secret env values only
  secret_refs_json TEXT NOT NULL,-- names -> keychain account keys; no values
  enabled INTEGER NOT NULL, approved_at TEXT, approved_fingerprint TEXT,
  last_status TEXT, last_error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE mcp_sync_entries (  -- what we wrote into each agent's config
  server_id TEXT NOT NULL, agent_id TEXT NOT NULL, config_path TEXT NOT NULL,
  entry_key TEXT NOT NULL, content_hash TEXT NOT NULL, backup_path TEXT,
  PRIMARY KEY (server_id, agent_id));
```
Rules: approval is bound to a fingerprint of (command, args, url); changing any invalidates approval. Test Connection and sync both require approval. Sync writes only entries recorded in `mcp_sync_entries`; if the on-disk entry's hash differs from what we wrote, it is treated as user-modified and not overwritten. Agent config formats are verified in spike C/M6; until verified an adapter reports "unsupported".

## 9b. Skill management architecture (ADR 0003)

```text
Source (git URL | local folder) ─► fetch into staging (no execution) ─► validate/inspect ─► user confirms
   ─► central store: <app_data>/skills/<id>/<revision>/ ─► SyncAdapter[agent]: symlink into agent skill dir
```
```sql
CREATE TABLE skills (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, source_kind TEXT NOT NULL, -- git | local
  source_url TEXT, revision TEXT, store_path TEXT NOT NULL, category TEXT,
  installed_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE skill_links (
  skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE, agent_id TEXT NOT NULL,
  link_path TEXT NOT NULL, enabled INTEGER NOT NULL, PRIMARY KEY (skill_id, agent_id));
```
Rules: git fetch uses the `git` CLI with argument arrays and no hooks/submodule execution (`core.hooksPath=/dev/null`); the app never runs scripts in a skill; a link is created only if the target path does not exist; uninstall removes only links we created (verified to point into our store) and the store directory. Update fetches to a new revision directory and swaps links atomically so rollback is possible.

## 10. Git service

Decision (ADR 0005): shell out to the installed `git` with argument arrays; read-only in the MVP.

Requirements:
- Detect repository root.
- Retrieve branch and working tree status.
- List changed files and status categories.
- Retrieve per-file diff with output limits.
- Handle repositories with large files, submodules, merge conflicts, detached HEAD, and missing Git executable.
- Do not run destructive Git commands in the background.
- All future mutating Git operations require explicit user action and confirmation.

## 11. Tauri IPC contract

Use a small, typed command surface. Suggested commands:

```text
workspace_list
workspace_create
workspace_rename
workspace_remove
workspace_open

terminal_spawn
terminal_write
terminal_resize
terminal_terminate
terminal_list_active

agent_detect
agent_list_adapters
agent_launch
agent_terminate
agent_get_capabilities

git_status
git_diff            # P1

mcp_list / mcp_upsert / mcp_remove / mcp_approve / mcp_test
mcp_sync_plan / mcp_sync_apply / mcp_sync_rollback

skill_list / skill_inspect / skill_install / skill_update / skill_uninstall
skill_set_enabled

settings_get
settings_update
```

Suggested events:

```text
session://created
session://lifecycle
terminal://output
terminal://error
agent://event
git://changed
```

IPC rules:
- Validate all arguments on the Rust side.
- Apply least-privilege Tauri capabilities.
- Use bounded payloads and output chunks.
- Do not emit secrets or full environment objects.
- Ensure event listeners are unsubscribed when views unmount.
- Do not expose generic arbitrary filesystem or process execution commands to the frontend.

## 12. Frontend architecture

Suggested feature layout:

```text
apps/desktop/src/
├── app/
├── components/
├── features/
│   ├── workspaces/
│   ├── sessions/
│   ├── terminal/
│   ├── agents/
│   ├── git-inspector/
│   ├── command-palette/
│   └── settings/
├── hooks/
├── stores/
├── services/
├── types/
└── styles/
```

State ownership:
- Rust owns process truth and durable metadata.
- SQLite owns persisted workspace/session metadata.
- Zustand owns ephemeral UI state: selected workspace, active tab, pane layout, inspector visibility.
- xterm.js owns terminal rendering buffer in memory.
- Do not put terminal output into Zustand or React component state.

Terminal component:
- Mount xterm.js once per active view/session.
- Use refs for terminal instances.
- Batch output writes.
- Dispose listeners and terminal instances correctly.
- Avoid remounting terminal on unrelated UI state changes.

## 13. Security model

Threats:
- Malicious project files or repository content.
- Prompt/terminal output containing instruction-like text.
- Custom CLI command injection.
- Accidental environment variable disclosure.
- Overly broad filesystem/process permissions.
- Dependency and adapter supply-chain risks.

Controls:
- Treat all project and agent output as untrusted data.
- Never execute commands found in README, config, agent output, or repository files automatically.
- Use structured executable/argument arrays.
- Require user-initiated workspace selection.
- Restrict Tauri capabilities to required operations.
- Avoid logging terminal data.
- Redact error details that may contain secrets.
- Review dependencies and licenses.
- Publish vulnerability reporting instructions.

## 14. Testing strategy

### Rust unit tests
- Workspace validation.
- Session state transitions.
- Adapter launch-spec construction.
- Database migrations and repository operations.
- Git status parsing.

### Rust integration tests
- Spawn PTY shell.
- Send input and receive output.
- Resize PTY.
- Exit and terminate processes.
- Missing executable and invalid working directory.
- Child process cleanup.

### Frontend tests
- Workspace and session state.
- Command palette actions.
- Agent status rendering.
- Terminal attach/detach lifecycle.
- Git inspector states.

### End-to-end tests
- Create workspace.
- Open terminal.
- Execute a harmless command.
- Open second tab and split.
- Launch a mock CLI agent.
- Verify lifecycle update.
- Restart app and verify metadata recovery.
- Confirm no transcript is written to persistent storage.

Use mock agent executables in CI; do not require paid provider accounts for tests.

## 15. Observability

Local diagnostics may include:
- Application version.
- OS version.
- Session IDs (random, local only).
- Process exit code.
- Adapter ID and version when available.
- Non-sensitive error category.

Never include:
- Terminal output or input.
- Prompts.
- Source code or file contents.
- Environment variable values.
- Access tokens or API keys.

## 16. Build and release

- macOS development and CI build instructions.
- Signed/notarized distribution process documented when distribution begins.
- Reproducible dependency lockfiles.
- Release notes and changelog.
- Apache-2.0 license file.
- Third-party notices and dependency license review.
- Security policy and contribution guide.

## 17. Technical spikes before full implementation

### Spike A — xterm.js + PTY
Prove:
- interactive zsh works,
- ANSI and common escape sequences,
- resize,
- paste and control keys,
- sustained output,
- process cleanup.

### Spike B — libghostty feasibility
Evaluate:
- integration surface and bindings,
- native rendering integration with Tauri,
- macOS packaging,
- licensing,
- maintainability,
- performance against xterm.js.

### Spike C — Agent CLI compatibility
For each supported CLI, test:
- executable detection,
- launch command and working directory,
- interactive behavior in PTY,
- process exit detection,
- documented structured event availability,
- version compatibility.

### Spike D — Process lifecycle
Test:
- closing a tab,
- closing a workspace,
- app shutdown,
- crash/restart behavior,
- orphan child processes.

## 17a. Intel and Apple Silicon

See PERFORMANCE_AND_COMPATIBILITY.md. Architecture claims are made only after a recorded run on real hardware of each architecture.

## 18. Implementation order

Superseded by [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) (milestones M0–M11 with scope, dependencies, acceptance criteria, tests).

## 19. Open technical questions

- Which PTY crate and process-supervision approach best meet current Tauri 2/macOS requirements?
- ~~Git executable vs library~~ — resolved by ADR 0005.
- Exact MCP config format and skill directory for each agent (verified in M6).
- What exact structured interfaces are officially supported by each agent CLI at implementation time?
- Can libghostty be integrated cleanly into the intended Tauri rendering architecture?
- What is the desired behavior when a workspace is removed while sessions are active?
- Should app close terminate all child processes, prompt the user, or support a configurable policy?

Resolve these through small spikes and decision records. Do not silently assume answers.
