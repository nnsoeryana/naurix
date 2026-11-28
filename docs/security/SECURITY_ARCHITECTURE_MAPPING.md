# Naurix Security Suite — Architecture Mapping

**Status:** Investigation only (2026-11-28). Nothing here is implemented, and nothing here is an accepted decision. Read-only audit of branch `dev` at `e191592`.
**Companion documents:** [SECURITY_INTEGRATION_MATRIX.md](SECURITY_INTEGRATION_MATRIX.md), [SECURITY_ARCHITECTURE_GAPS.md](SECURITY_ARCHITECTURE_GAPS.md).

This document maps each concept in the Security Suite brief to what exists in the repository today. Each claim names a file and, where useful, a line. "Brief" means the Security Suite request; its terms (for example "Core Permission Engine", "shared context") are matched to the closest real component, and the mismatch is stated when there is one.

---

## 0. Corrections to the brief's assumptions

The brief lists "confirmed architecture decisions". The code shows several of them rest on names or capabilities that do not exist in this form:

| Brief says | What the repository has | Evidence |
|---|---|---|
| "Follow existing TypeScript conventions" | Behaviour lives in **Rust** (`crates/core`); TypeScript is the React UI only. AGENTS.md: "Keep process execution and PTY management in Rust", "Keep domain logic independent from React and Tauri command handlers". | `Cargo.toml` (workspace `crates/core`, `src-tauri`), `crates/core/src/lib.rs:1` ("Framework-independent core (ADR 0002). Must not depend on Tauri.") |
| "Core Permission Engine" | A **central approval gate**: fingerprint-bound, scoped grants for a closed set of action classes. It is not a generic permission/policy engine. | `crates/core/src/approval/mod.rs:31` (`ActionClass`), `:56` (`Scope`), ADR 0027 |
| "Shared context and memory synchronization" | (a) A **lossy, metadata-only** event bus and (b) **user-authored** notes per workspace. There is no shared context store, no automatic capture, and no synchronization between agents. | `crates/core/src/events.rs:1-6`, `crates/core/src/memory/mod.rs`, ADR 0031 §8 |
| "DAG Workflow Engine" | Exists, but a step is **only a reference to an agent**: no prompts, no input, no data passed between steps, completion = process exit code or the user's "Mark done". | `crates/core/src/workflow/mod.rs:51` (`StepInput`), `crates/core/src/workflow/engine.rs:1-3`, ADR 0030 §4, §7 |
| "Skill Management and skill loading" | Skills are **installed, inspected, copied and symlinked into agent folders; never loaded or executed by Naurix**. Naurix itself does not "load" a skill. | `crates/core/src/skills/mod.rs:3-5`, `crates/core/src/skills/scan.rs:1-2`, ADR 0016 §2 |
| "Hybrid CLI + MCP through adapter abstraction" | `AgentAdapter` abstracts **coding agents** (Claude Code, Codex, …), not tools. Naurix speaks MCP only for **Test Connection** (`initialize` + `tools/list`); it never calls `tools/call`. | `crates/core/src/agents/behavior.rs:46`, `crates/core/src/mcp/probe.rs:1-3` |
| "Existing project/session storage" | One SQLite DB with append-only migrations. `sessions` rows are **metadata only**. Storing findings means storing content, which the metadata-first invariant forbids by default. | `crates/core/src/persistence/mod.rs:61-256`, AGENTS.md "Metadata-first persistence" |

---

## 1. Repository structure and package boundaries

| Unit | Path | Role | Notes |
|---|---|---|---|
| Rust workspace | `Cargo.toml` | Two members: `crates/core`, `src-tauri` | release profile `opt-level="s"`, `panic="abort"` |
| Core crate `naurix-core` | `crates/core` | All domain logic; no Tauri dependency (ADR 0002) | deps: `portable-pty`, `rusqlite` (bundled, FTS5), `serde`, `sha2`, `thiserror`; keychain crates per OS |
| Tauri shell `naurix` | `src-tauri` | Bridge: validates input, calls Core, adapts to Channels/events | command list `src-tauri/src/lib.rs:203-283` |
| Web UI | `src/` | React 19 + Zustand + xterm.js + `@xyflow/react` | feature folders `src/features/*`, each with `api.ts` wrapping `invoke` |
| Docs | `docs/`, `docs/adr/` (0001–0031) | PRD, technical design, ADRs | ADRs are immutable once accepted |
| Embedded skills | `crates/core/resources/skills/` | `natural-writing` only | `crates/core/src/skills/builtin.rs:15` |

Core modules (`crates/core/src/lib.rs:3-17`): `agents`, `approval`, `credential`, `events`, `git`, `mcp`, `memory`, `persistence`, `platform`, `pty`, `session`, `settings`, `signals`, `skills`, `workflow`.

There is **no** `security` module, no SARIF/finding type, and no mention of scanners anywhere in code (`grep` for `sarif|semgrep|vulnerab` in `crates`, `src`, `src-tauri`: no hits).

## 2. Agent Runtime

Layering (ADR 0022 §4.3, PRD ORCH-001): **Terminal Runtime** (`crates/core/src/pty`) knows nothing about agents; **Agent Runtime** (`agents`, `session`, `approval`, `signals`, `workflow`) sits above it.

| Piece | Location | What it does |
|---|---|---|
| Agent definitions | `crates/core/src/agents/def.rs:22` `PRESETS` (claude-code, codex, gemini-cli, antigravity-cli, opencode); `:70` `CustomAgent { name, executable, args, env }` | A custom agent is any executable + argument array; env names that look like secrets are rejected (`def.rs:95` `SECRET_HINTS`, enforced at `:147`) |
| Adapter facts | `crates/core/src/agents/adapter.rs:12` `Feature { PtyLaunch, McpConfig, Skills, StatusSignals, SessionResume }`, `:22` `Level { Verified, DocumentedOnly, DocumentedAccepted, Unsupported }`, `:92` `acts_on` | Per-capability verification; Naurix acts only on `Verified`/`DocumentedAccepted` |
| Adapter behaviour | `crates/core/src/agents/behavior.rs:46` `trait AgentAdapter { info, mcp_target, skill_compat, status_args }`; registry `:182` (5 presets); custom agents have no adapter (`:185`) | No headless/structured execution method exists ("added when their first capability is verified", `behavior.rs:5-6`) |
| Launch planning | `crates/core/src/agents/def.rs:170` `launch_action` (approval fingerprint over id, program, args, env), `:239` `plan_launch` | Display-only command string; never executed as a shell string |
| Launch path (shell) | `src-tauri/src/agent.rs:257` `resolve`, `:293` `build_launch`, `:418` `agent_launch` | MCP secrets injected **only** for adapters that act on `McpConfig` (`agent.rs:304-310`) — never for custom agents |
| Session ownership | `crates/core/src/session/mod.rs:206` `SessionManager`; `:66` `SessionLaunch`; `:297` `start`; `:271` `ensure_agent_slot` (limit, default 4) | One stable id; sessions survive UI reload (ADR 0023, 0026) |
| Status signals | `crates/core/src/signals.rs:29` `SignalKind { Working, AwaitingApproval, Idle, TurnDone }` | Claude Code only, via `--settings` hooks into a spool file (`behavior.rs:78-87`, ADR 0029) |

**Key fact for the suite:** Naurix cannot send a prompt to an agent. There is no `SendInput` action class (`approval/mod.rs:31-40`; ADR 0027 §2 lists it as "later"), and workflow steps carry no prompt (ADR 0030 §4). A "Red Team agent" today is a launched CLI agent the user types into.

## 3. Workflow Engine and DAG execution

| Piece | Location | Behaviour |
|---|---|---|
| Model + validation | `crates/core/src/workflow/mod.rs:16-18` (`MAX_STEPS=24`, `MAX_EDGES=64`, names ≤60), `:94` `validate` (cycle check in Core) | Step = `{name, agent_id, depends_on}` (`:51`) |
| States | `mod.rs:157` `StepState`, `:198` `RunState`, `:228` fixed `note` vocabulary | `satisfies()` = succeeded or skipped |
| Store | `crates/core/src/workflow/store.rs:7` `WorkflowStore`, `:136` `RunStore` | Runs snapshot steps and edges (migration 6, `persistence/mod.rs:193-240`) |
| Engine | `crates/core/src/workflow/engine.rs:48` `Engine`; `:27` `trait Launcher { action, launch }`; `:78` `spawn_listener` (reacts to `SessionExited`); `:185` `start`; `:379/396/413` mark_done/skip/cancel | Steps launched detached; exit 0 = succeeded, anything else = failed; dependents of failed steps are **blocked** |
| Shell launcher | `src-tauri/src/workflow.rs:27` `ShellLauncher` | Reuses the user launch path |
| Approval | ADR 0030 §3: one `run_workflow` once-grant → run-scoped `launch_agent` grants (`approval/mod.rs:386` `authorize_in_run`, `:396` `grant_run`, `:416` `end_run`) |

Explicit limits (ADR 0030 §7): no shell-step type (use a custom agent), no conditions, loops, retries, schedules, triggers, parameters, variables, or data flow between steps.

## 4. Skill management

| Piece | Location | Behaviour |
|---|---|---|
| Sources | `crates/core/src/skills/store.rs:209` `inspect_git` (https only, shallow, hooks off — `skills/git.rs:17,40`), `:237` `inspect_local` | A source may contain **many** skills: `scan.rs:59` `find_skill_dirs` searches `SKILL.md` to depth 3, up to 100 candidates |
| Inspection | `scan.rs:223` `inspect_candidate`, `:24` `Flag` kinds (`shell_injection`, `allows_shell_tools`, `defines_hooks`, `scripts`, `binary_files`, `symlinks_skipped`) | Untrusted; never executed |
| Install | `store.rs:361` `install` — per-selection tree-hash check | Central copy `<data>/skills/<id>/rev-<hash>/` + `current` symlink |
| Link to agent | `store.rs:543` `set_link`; per-agent folders from adapter `config_paths` | Symlink only where nothing exists; `DocumentedAccepted` level (ADR 0016 §1) |
| Update/revert/uninstall | `store.rs:632/703/748/789` | Atomic re-point of `current` |
| Built-in | `skills/builtin.rs:15` | Seeded once, never auto-linked |

A skill is SKILL.md + files; Naurix understands only front-matter `name`/`description` (`skills/frontmatter.rs`). A skill cannot declare workflows, custom agents, MCP servers, or finding schemas.

## 5. Project/session storage

- Single SQLite DB, `Db` (`crates/core/src/persistence/mod.rs:266`), migrations appended to `MIGRATIONS` (`:62`, "Never edit an existing entry; append a new one"), `user_version` pragma (`:524-534`). Currently 7 migrations.
- Tables: `workspaces`, `sessions`, `settings` (1); `mcp_servers`, `mcp_sync_entries`, `mcp_sync_runs` (2); `skills`, `skill_links` (3); `approvals` + `sessions.approval_id` (4); `worktrees` + `sessions.worktree_id` (5); `workflows`, `workflow_steps`, `workflow_deps`, `runs`, `run_steps`, `run_deps` (6); `memory_notes`, `memory_fts`, `workspaces.memory_enabled` (7).
- Every table stores **metadata**; the single content exception is `memory_notes` (ADR 0022 §3, AGENTS.md).
- Settings are a key/value JSON table (`persistence/mod.rs:378-416`) plus a typed model (`crates/core/src/settings.rs`).
- Restart marks running sessions `interrupted` (`persistence/mod.rs:471`) and runs `interrupted` (`workflow/engine.rs:432`).

## 6. Shared context and memory

- `crates/core/src/memory/mod.rs:129` `MemoryStore`: opt-in per workspace (`:170`), notes ≤100/8000 chars, ≤200 per workspace (`:14-17`), FTS5 search with sanitized query (`:119` `fts_query`), export Markdown/JSON (`:285`), `inside_git_repo` check (`:315`).
- `crates/core/src/memory/secrets.rs:119` `scan`: rejects credential-shaped text, no override.
- **Only the user creates notes** (ADR 0031 §8). Delivery to an agent is a user click that pastes into the active agent pane without Enter (ADR 0031 §6).
- There is no cross-agent context, no context window assembly, no retrieval API for agents.

## 7. Event-driven communication

- `crates/core/src/events.rs:18` `Event { SessionStarted, SessionExited, RunChanged, SessionStatus }`; `:46` `EventBus` with `subscribe` / `publish` on bounded `sync_channel(256)` queues; full queues **drop** events (`:62-71`).
- Contract (`events.rs:3-6`): ids and fixed kinds only — never bytes, prompts, titles, env or file content; "nothing may depend on an event for correctness".
- Bridged to the webview in `src-tauri/src/events.rs` (`run://changed` etc.).

## 8. Permission and approval

- `crates/core/src/approval/mod.rs`: `ActionClass { LaunchAgent, CreateWorktree, RemoveWorktree, RunWorkflow }` (`:31`), `Scope { Once, Workspace, App, Run }` (`:56`), `fingerprint` (`:102`, length-prefixed SHA-256), `Action` built only by the backend (`:113`), `ApprovalRequest` without fingerprint (`:153`), `authorize`/`preview`/`decide`/`revoke` (`:277-545`), 5-minute TTL, max 32 pending (`:23-24`), history 200 (`:26`).
- Table `approvals` (migration 4, `persistence/mod.rs:162-177`).
- Other, not-yet-migrated approval mechanisms: MCP server fingerprint (`mcp/model.rs:322` `fingerprint`, `:338` `is_approved`; `mcp/store.rs:140` `approve`), MCP/skill sync plan hash (`mcp/sync.rs:200` `plan`, `:403` `apply`, `:521` `rollback`), Git repository trust (`git/mod.rs:104-138`).
- Tauri: `approval_decide/list/revoke/revoke_all` (`src-tauri/src/lib.rs:239-242`).
- Adding a class = add the enum variant with the feature that needs it (`approval/mod.rs:28`).

## 9. CLI / terminal execution

- `crates/core/src/pty/spec.rs:11` `SpawnSpec` (program + args array; `MAX_INPUT_BYTES` 1 MiB; `:58` `resolve_executable`), `pty/supervisor.rs:23` `Supervisor`, `pty/session.rs:69` `PtySession` (terminate ladder `:239`), `pty/forward.rs` (flow control, 64 KiB batches, 256 KiB high water).
- Output goes to a bounded in-memory replay ring (`session/mod.rs:32` `DEFAULT_REPLAY_BYTES = 1 MiB`) and to the view; it is **never persisted or parsed** by Core.
- OS differences confined to `crates/core/src/platform.rs` (AGENTS.md).
- Non-PTY child processes exist only for Git (`git/run.rs:53`, timeout 20 s, output cap 8 MiB), MCP probe (`mcp/probe.rs:58`) and `--version` detection (`agents/version.rs:11`).

## 10. MCP integration

- Registry (`mcp/store.rs:41` `McpStore`), model (`mcp/model.rs:37` `McpInput`, transports stdio/http), secrets in OS keychain (`mcp/secrets.rs:6` `SecretStore`, service `dev.naurix.app.mcp`), static catalog (`mcp/catalog.rs:32`).
- Test Connection (`mcp/probe.rs`): stdio only, approved servers only, `initialize` + `tools/list`, kills the process tree after.
- Sync into agent configs (`mcp/sync.rs`): dry-run plan, approval bound to plan hash, backup, apply, rollback; targets only `Verified` formats (Claude `.mcp.json`, OpenCode `opencode.jsonc` — `agents/behavior.rs:16-21`).
- Agents call MCP tools themselves; Naurix never does.

## 11. Test infrastructure

- Rust integration tests `crates/core/tests/*.rs` (adapters, agents, approval, events, git, mcp, memory, pty, pty_windows, session, session_limit, session_reattach, skills, status, supervisor_race, workflow, worktree); unit tests inline (`#[cfg(test)]`); fixtures from real CLIs in `crates/core/tests/fixtures/agents` (e.g. `claude-2.1.322-project.mcp.json`).
- Patterns to reuse: scripted mock agents (`tests/workflow.rs:26` `script`), mock `Launcher` (`tests/workflow.rs:39-95`), in-memory DB (`persistence/mod.rs:275` `open_in_memory`), planted-marker tests that prove content never reaches the DB (ADR 0030 log) and that skill scripts never run (ADR 0016 §2).
- Frontend: Vitest `*.test.ts` beside modules (`src/features/*/`); dev smoke harness `src/dev/smoke.ts`.
- CI (`.github/workflows/ci.yml`): macOS/Linux/Windows; `pnpm typecheck`, `pnpm test`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`.

## 12. Coding conventions

- Core: `thiserror` error enums per module with user-actionable messages (e.g. `approval/mod.rs:80-88`); `From<DbError>`/`From<rusqlite::Error>` mapping; stores as `XStore<'a> { db: &'a Db }`; serde `rename_all = "snake_case"`/`"camelCase"`; ids via `persistence::new_id`; ISO-8601 via `persistence::time`; length limits as `pub const`; control-character stripping (`workflow/mod.rs:78` `clean_name`).
- Tauri: commands return `Result<T, String>`; the webview names ids and user text, never programs/args/paths (`src-tauri/src/workflow.rs:1-3`).
- UI: `src/features/<feature>/{api.ts, View.tsx, model.ts, *.test.ts}`; terminal output kept out of React state (AGENTS.md).
- Process: each feature lands with an ADR (Context → Decision → Consequences), an implementation log with an explicit "Not verified" list, and PRD requirement IDs.
- Dependencies are added only after a maintenance/license/bundle check (AGENTS.md; example in ADR 0030 "Run graph").
