# Naurix Security Suite — Integration Matrix

**Status:** Investigation only (2026-11-28). Classifications describe the code on `dev` at `e191592`; none is an accepted decision.
**Legend:**
- **ALREADY SUPPORTED** — works today with configuration only, no code change.
- **REQUIRES EXTENSION** — an existing module gains a variant, column, method or UI; its shape and invariants are kept.
- **REQUIRES NEW IMPLEMENTATION** — no existing component does it; new Core code (and an ADR) is needed.
- **UNKNOWN / NEEDS INVESTIGATION** — depends on facts not in the repository (agent behaviour, tool behaviour, maintainer decision).

Evidence paths are relative to the repository root. Details of each module: [SECURITY_ARCHITECTURE_MAPPING.md](SECURITY_ARCHITECTURE_MAPPING.md). Conflicts and missing capabilities: [SECURITY_ARCHITECTURE_GAPS.md](SECURITY_ARCHITECTURE_GAPS.md).

---

## A. Execution (Hybrid CLI + MCP)

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| A1 | Run a security CLI (e.g. a SAST/SCA/secret scanner) in a PTY in the workspace folder | **ALREADY SUPPORTED** | `agents/def.rs:70` `CustomAgent`, `agents/def.rs:239` `plan_launch`, `session/mod.rs:297` `start` | Defined as a custom agent: executable + args array; reviewed through the gate on launch |
| A2 | Run it as a workflow step | **ALREADY SUPPORTED** | ADR 0030 §7 ("use a custom agent"), `workflow/engine.rs:27` `Launcher` | Counts toward the agent limit (`session/mod.rs:37`) |
| A3 | Treat "findings found" exit codes as success | **REQUIRES EXTENSION** | `workflow/mod.rs:157` `StepState`; ADR 0030 §2 (non-zero = failed, dependents blocked) | Many scanners exit non-zero when they find issues. Today the user must Skip/Mark done, or wrap the tool. A per-agent "accepted exit codes" field is the smallest extension |
| A4 | Pass secrets (scanner API tokens) to a security CLI | **REQUIRES EXTENSION** | `agents/def.rs:95` `SECRET_HINTS`, checked in `validate_custom` (`:105`, `:147`) rejects secret-like env names; `src-tauri/src/agent.rs:304-310` injects keychain secrets only for adapters acting on `McpConfig` | No keychain path for custom agents today |
| A5 | "Tool adapter abstraction" (one interface for CLI and MCP security tools) | **REQUIRES NEW IMPLEMENTATION** | `agents/behavior.rs:46` `AgentAdapter` is for coding agents; custom agents have no adapter (`behavior.rs:185`) | Don't overload `AgentAdapter`; see Gaps G5 |
| A6 | Use a security MCP server **through an agent** (agent calls the tool) | **ALREADY SUPPORTED** | `mcp/store.rs` registry + approval, `mcp/sync.rs` dry-run/apply/rollback into Claude Code / OpenCode configs | Only for agents with `McpConfig` Verified (`agents/adapter.rs`, `docs/AGENT_ADAPTERS.md`) |
| A7 | Naurix itself calls MCP `tools/call` | **REQUIRES NEW IMPLEMENTATION** | `mcp/probe.rs:1-3` (only `initialize`, `tools/list`) | Needs a new action class and an ADR; conflicts with "Naurix never acts as the agent" posture (Gaps C4) |
| A8 | Probe a security MCP server's tool list | **ALREADY SUPPORTED** | `mcp/probe.rs:58` `probe`, `src-tauri/src/mcp.rs` `mcp_test` | stdio only, approved servers only |
| A9 | Headless/non-interactive agent execution (agent runs a security prompt unattended) | **UNKNOWN / NEEDS INVESTIGATION** | `agents/adapter.rs:12` `Feature` has no headless variant; `behavior.rs:5-6` | Needs a per-agent verified capability with fixture + version (ADR 0014, 0022 §1.2) |
| A10 | Sending a security prompt into an agent automatically | **REQUIRES NEW IMPLEMENTATION** | `approval/mod.rs:31` (no `SendInput`), ADR 0027 §2, ADR 0030 §4 | Also needs a metadata-first exception (prompts) — Gaps C2 |

## B. Agent Runtime

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| B1 | Launch an AI agent for a Red/Blue/Purple role (user types the instructions) | **ALREADY SUPPORTED** | `src-tauri/src/agent.rs:418` `agent_launch`, preset or custom agent | "Role" is only a name/args on a custom agent today |
| B2 | Role identity (Red/Blue/Purple) as first-class metadata | **REQUIRES EXTENSION** | `persistence/mod.rs:71-83` `sessions` (kind, adapter_id); `SessionKind { Terminal, Agent }` `persistence/mod.rs:35` | Smallest: a role tag on custom agents or workflow steps; avoid a new `SessionKind` |
| B3 | Concurrency control across teams | **ALREADY SUPPORTED** | `session/mod.rs:271` `ensure_agent_slot`, `settings.rs:143-146` | Global limit, not per team |
| B4 | Run an agent in an isolated worktree (e.g. Blue team remediation branch) | **ALREADY SUPPORTED** (manual) / **REQUIRES EXTENSION** (in workflows) | ADR 0028; `git/worktree.rs:87` `plan_create`; ADR 0030 §3 excludes worktrees from workflows | Worktree creation is once-only, cannot be pre-approved for a run |
| B5 | Verified status (working / awaiting approval / done) for security agents | **ALREADY SUPPORTED** (Claude Code only) | `signals.rs:29`, `agents/behavior.rs:78-87`, ADR 0029 | Not for custom CLIs; never use it to complete a step (ADR 0030 §2) |

## C. Workflow / DAG

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| C1 | Red → Blue → Purple ordering as a DAG | **ALREADY SUPPORTED** | `workflow/mod.rs:94` `validate`, `workflow/engine.rs:185` `start` | Ordering only |
| C2 | Pass findings from one step to the next | **REQUIRES NEW IMPLEMENTATION** | ADR 0030 §7 (no variables/data flow); `workflow/mod.rs:51` | Today only via files in the workspace folder, which Naurix does not read |
| C3 | Conditional branches (e.g. run Purple validation only if findings exist) | **REQUIRES NEW IMPLEMENTATION** | ADR 0030 §7 (no conditions) | Defer |
| C4 | Security regression run (re-run a saved workflow) | **ALREADY SUPPORTED** | ADR 0030 ("runnable many times") | Manual trigger only; no schedule/trigger (ADR 0030 §7) |
| C5 | > 24 steps or parameterized targets | **REQUIRES EXTENSION** | `workflow/mod.rs:16-18` | Not needed for a first slice |
| C6 | Ship ready-made security workflows | **REQUIRES NEW IMPLEMENTATION** | No workflow import/export exists (`src-tauri/src/lib.rs:218-225` command list) | Needs an import format + review; see D3 |

## D. Distribution (standalone skill pack)

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| D1 | Install a repo/folder containing several security skills (Red/Blue/Purple) | **ALREADY SUPPORTED** | `skills/scan.rs:59` `find_skill_dirs` (depth 3, ≤100 skills), `skills/store.rs:209,237,361` | https Git or local folder; reviewed, hash-checked, never executed |
| D2 | Link those skills into Claude Code / Codex / OpenCode / Antigravity | **ALREADY SUPPORTED** (link only, discovery not verified) | `skills/store.rs:543` `set_link`; ADR 0016 §1 `DocumentedAccepted` | Gemini CLI unsupported |
| D3 | Pack also carries custom agents, workflows, MCP definitions | **REQUIRES NEW IMPLEMENTATION** | `skills/frontmatter.rs` reads only `name`/`description`; no manifest type | Would need a manifest + per-item review via existing approvals |
| D4 | Pack includes scanner scripts that Naurix runs | **CONFLICT — not allowed** | AGENTS.md "Never execute skill scripts"; ADR 0016 §2 | Scripts may be run only by an agent the user launched, or by an explicitly defined custom agent |
| D5 | Ship the pack built-in | **REQUIRES EXTENSION** | `skills/builtin.rs:15` `BUILTIN` | Contradicts "standalone"; not recommended |

## E. Storage

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| E1 | Record which security run/step ran, when, exit code | **ALREADY SUPPORTED** | `runs`, `run_steps` (migration 6, `persistence/mod.rs:193-240`) | Metadata only |
| E2 | Persist findings (rule id, severity, file, line, message, fingerprint) | **REQUIRES NEW IMPLEMENTATION** + ADR exception | AGENTS.md "Metadata-first persistence"; ADR 0022 §3.3 | Findings are content derived from source and tool output. Needs a new ADR and migration 8 in the **same** DB |
| E3 | Store raw scanner output | **CONFLICT — not allowed** | AGENTS.md; `session/mod.rs:32` (in-memory replay only) | Keep only normalized, bounded, user-confirmed fields, if any |
| E4 | Finding triage state (open / accepted risk / fixed / false positive) | **REQUIRES NEW IMPLEMENTATION** | — | Depends on E2 |
| E5 | Settings for the suite (enabled, allowed targets) | **REQUIRES EXTENSION** | `settings.rs:180` `Settings`, `persistence/mod.rs:378-416` | Follow `Agents` section pattern |

## F. Context, memory, events

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| F1 | User writes a security note (threat model, accepted risks) | **ALREADY SUPPORTED** | `memory/mod.rs:189` `save` | Opt-in per workspace; secret scanner refuses credential-shaped text (`memory/secrets.rs:119`) — this may refuse legitimate security notes quoting token shapes |
| F2 | Automatically capture findings into memory | **CONFLICT — not allowed** | ADR 0022 §3.3, ADR 0031 §8 | |
| F3 | Insert a note into an agent terminal | **ALREADY SUPPORTED** | ADR 0031 §6 | User-driven, no Enter |
| F4 | Events for finding created/changed | **REQUIRES EXTENSION** | `events.rs:18` `Event` | Ids only (e.g. `FindingsChanged { workspace_id, run_id }`); lossy, so UI re-reads |
| F5 | Agent-to-agent shared context (Red team output visible to Blue) | **REQUIRES NEW IMPLEMENTATION** | Not present; events carry no content (`events.rs:3-6`) | Only via E2 store, read by the user/UI |

## G. Permissions / approval

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| G1 | Approve launching a security CLI or agent | **ALREADY SUPPORTED** | `agents/def.rs:170` `launch_action`, `approval/mod.rs:277` | Fingerprint covers program, args, env |
| G2 | Approve a whole security workflow run | **ALREADY SUPPORTED** | `ActionClass::RunWorkflow`, `approval/mod.rs:386-416` | |
| G3 | Target-scope authorization for offensive (Red) actions (which hosts/URLs may be tested) | **REQUIRES NEW IMPLEMENTATION** | No target concept in `approval/mod.rs` | Needs a new action class, e.g. `RunOffensiveCheck`, once/run scope, fingerprint includes target list |
| G4 | Blue-team remediation writes to code | **ALREADY SUPPORTED** (by agent, under its own permissions) | Naurix never writes source; agents do | Naurix must not apply patches itself without a new once-only class (ADR 0022 §4.2 analogue) |
| G5 | Git mutation for fixes (branch/commit) | **CONFLICT unless once-approved** | AGENTS.md "do not mutate Git state"; ADR 0028 | Only worktree create/remove classes exist |

## H. UI

| # | Integration | Class | Evidence | Notes |
|---|---|---|---|---|
| H1 | Security view (teams, runs, findings) | **REQUIRES NEW IMPLEMENTATION** | `src/features/*` pattern; `src/features/workflows/` as template | Must not be required to use the terminal |
| H2 | Run graph for a security workflow | **ALREADY SUPPORTED** | `src/features/workflows/RunGraph.tsx` | |
| H3 | Approvals listing | **ALREADY SUPPORTED** | `src/features/approvals/ApprovalsPanel.tsx` | New classes appear automatically if they use the gate |

## I. Tests and docs

| # | Integration | Class | Evidence |
|---|---|---|---|
| I1 | Mock scanner as scripted agent in Core tests | **ALREADY SUPPORTED** | `crates/core/tests/workflow.rs:26-95` |
| I2 | "No content reaches the DB" planted-marker test for findings | **REQUIRES EXTENSION** | ADR 0030 log pattern |
| I3 | Fixtures from real scanners (SARIF samples with tool version) | **REQUIRES NEW IMPLEMENTATION** | `crates/core/tests/fixtures/agents/README.md` pattern |
| I4 | ADR + PRD requirement IDs | **REQUIRES NEW IMPLEMENTATION** | `docs/adr/README.md`, PRD §5.9 |
