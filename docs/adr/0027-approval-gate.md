# ADR 0027 — Central approval gate (orchestration stage 5)
**Status:** Accepted (2026-11-23, maintainer). **Implemented; see the implementation log at the end for what is not yet verified.** Implements the approval-gate half of stage 5 of ADR 0022 and proposes moving the Task/Run model to stage 8 (see §6).

## Context (from the code, 2026-11-23)
ADR 0022 requires one centralized, scoped approval gate for every agent action. Today there are **four separate mechanisms**, each with its own storage and UI:

| Mechanism | Storage | What it approves | Revoke |
|---|---|---|---|
| Agent launch | `settings` key `agents.approved`: agent id → resolved program path | A preset is approved for its exact executable path **across all workspaces**; a custom agent is reviewed on every launch | Implicit: changing the path or editing a custom agent drops it |
| MCP server | `mcp_servers.approved_fingerprint` (SHA-256 of command, args, url) | Running a server definition; also gates Test Connection and sync | Per server and "revoke all" in Security |
| MCP / skill sync | `plan_hash` bound to the reviewed plan | One concrete write to an agent config or skills folder | n/a (one-shot) |
| Git repository trust | `settings` keys `git.trust.<hash>` | Letting Naurix run a repository's own configured Git commands | Per repository in Security |

**A design gap in the launch path.** `agent_launch` takes a `reviewed: bool` from the webview. The backend cannot tell what the user was actually shown, and a UI that passes `true` skips review. The MCP and sync mechanisms are better: the approval is bound to a fingerprint the backend computed.

There is also no record of which approval allowed which launch, no way to see launch approvals in Settings, and no limit on how many agents run at once (the only concurrency figure measured is four agents, `bench/results`).

## Decision

### 1. One gate in Core, driven by requests the backend builds
A new `crates/core/src/approval` module (no Tauri dependency). Flow, which generalises today's `ReviewRequired` then retry-with-`reviewed=true`:
1. The caller (for example `agent_launch`) asks the gate to **authorize** an `Action`. The action is built by the backend from resolved data (agent, absolute program path, argument array, additional environment names and values the user can see, workspace).
2. If a live grant matches the action's **fingerprint** (and scope, see §2), it is allowed.
3. Otherwise the gate returns `ApprovalRequired(request)`. The request holds a **server-built summary** (what the UI must show, for example the exact command line) and the fingerprint, and is kept in memory, **single-use, expiring after 5 minutes**.
4. The UI shows the summary and calls `approval_decide(request_id, deny | allow(scope))`. The UI never supplies a fingerprint or a summary; it only names a request and a scope.
5. The caller retries the action. The gate **recomputes the fingerprint** and compares it with the decided request, so a changed target cannot ride on an earlier decision.
This replaces the `reviewed: bool` parameter.

### 2. Scopes
Scopes are those fixed in ADR 0022: **once**, **session**, **run / workflow run**, **workspace**, plus one narrow addition decided for stage 5: **app**, usable **only for launching a preset agent**, bound to the exact resolved executable path. It exists to keep today's behaviour (a reviewed preset is not asked about again in another workspace). No other action class may use it, so it is not an "approve everything" scope. A grant stores `scope` and `scope_ref` (workspace id, and later a session or run id), so adding a scope later needs no new table. **Stage 5 enables only the scopes an action class uses now.** Requesting any other scope is rejected with a clear error.

Maximum scope per action class (ADR 0022 §4.2):

| Class | Max scope | Stage |
|---|---|---|
| `LaunchAgent` (preset) | app, for that exact executable path | **5** |
| `LaunchAgent` (custom) | workspace, chosen by the user; bound to the full fingerprint | **5** |
| `SendInput` | session / run | later, with automated input |
| `CreateWorktree` | once | stage 6 |
| `WriteAgentConfig` | once | when MCP/skill sync moves to the gate |
| `RunWorkflowStep` | run | stage 8 |

Only `LaunchAgent` exists as a class in stage 5. Other classes are added with the feature that needs them, not before.

### 3. What the fingerprint covers
SHA-256 over: action class, agent id, **absolute program path**, the argument array, and additional environment entries (name and value) that are visible in the review. It does **not** cover environment supplied by Naurix at launch (MCP secrets from the Keychain), which are never displayed or stored. It does **not** hash the executable's bytes: an updated binary at the same path stays approved, as it is today. This is stated in the review text and in docs, not hidden.

### 4. Storage and audit
One table `approvals` (migration): `id`, `class`, `scope`, `scope_ref`, `fingerprint`, `summary` (agent name and program path only; **no arguments or environment values are persisted in the summary**), `workspace_id` (nullable, `ON DELETE CASCADE`), `state` (`active` | `consumed` | `revoked`), `created_at`, `last_used_at`, `ended_at`. Metadata only, per ADR 0022.
- **Once** grants are consumed atomically (a conditional update that checks the row count), so a grant cannot be used twice, including under concurrent launches.
- Consumed and revoked rows are kept as a history, capped at the newest 200, so "which approval allowed this" can be answered. Active grants are never pruned.
- A new nullable column `sessions.approval_id` records which approval allowed an agent launch. This gives traceability without a new entity (see §6).
- Granting the same class, scope and fingerprint again updates the existing active row instead of adding another.
- Removing a workspace removes its grants.

### 5. Visibility and revocation
- Settings → Security lists active grants (class, target summary, scope, workspace, created, last used) with **Revoke**, and **Revoke all** (the existing MCP "revoke all" pattern).
- Changing a target (a new executable path, an edited custom agent) changes the fingerprint, so the old grant simply stops matching.
- The existing launch review dialog is replaced by an approval dialog fed from the request summary, with the scope choices the class allows.

### 6. Task and Run are not built in this stage
ADR 0022 lists "Task/Run plus approval gate" for stage 5. Looking at the code, a `runs` table would **duplicate `sessions`**: an agent session row already records kind, adapter, workspace, timestamps and lifecycle. Until workflows exist there is nothing for a Task or Run to relate (no dependency edges, no workflow runs). So:
- Stage 5 delivers the **gate**, **traceability** (`sessions.approval_id`) and a **concurrency limit**.
- The **Task / Run / dependency model moves to stage 8 (workflows)**, where it has a job to do. This amends the stage list in ADR 0022 and `AGENTS.md`; both are edited only after this ADR is accepted.

### 7. Concurrency limit
A setting `maxConcurrentAgents` (default **4**, the only figure measured so far; range 1 to 16) counts **running agent sessions** only. Plain terminals are never limited (terminal-first). At the limit, launching fails with a clear message and nothing starts. The limit is a resource guard, not a safety control, and is enforced in Core when a session starts, not in the UI.

### 8. Migration and scope of this stage
- Existing preset approvals (`agents.approved`) are **converted into gate grants on first start** so users are not re-prompted (they become **app**-scoped grants for that exact executable path). Custom agents were never stored as approved, so nothing converts.
- **Not migrated in this stage:** MCP server approval, MCP and skill sync plans, and Git repository trust. They approve a *definition* or a *plan*, not an action, and work correctly today. They are listed in Security as they are now, and move to the gate when their stage needs it. Gate grants appear in a new section of the same screen.

### 9. Security properties
- The webview cannot create a grant for anything the backend did not compute; it can only answer a request.
- A request is single-use and expires; a decided request is bound to the fingerprint it was created for.
- Grants are never created implicitly. Naurix does not auto-approve, there is no "approve everything" scope, and Git mutations and agent-config writes stay "once" only (ADR 0022).
- No terminal output, prompt or environment value is stored in `approvals`.

## Consequences
- Core gains `approval` plus a migration (new table, `sessions.approval_id`). `agents::AgentStore::prepare_launch` and the `reviewed` flag are replaced by the gate. `agent_launch` takes an optional request id instead of `reviewed`. Tauri adds `approval_decide`, `approval_list`, `approval_revoke`, `approval_revoke_all`. The launcher dialog and the Security screen change.
- Tests required: once-consumed-once (including concurrent use), workspace grant matches only the same fingerprint and workspace, a changed argument or path invalidates, expiry, a decided request cannot be reused or retargeted, revoke and revoke all, workspace removal removes grants, history cap, concurrency limit, conversion of legacy `agents.approved`, and the launch path end to end with the mock agent.
- ADR 0011 §6 and ADR 0007 §4 (review rules for launch) are superseded for the mechanism only; the rule "custom agents and the first launch of a preset are reviewed" still holds.
- Risk: a regression in a path every user takes (launching an agent). Mitigation: migration of existing approvals, the mock-agent end-to-end test, and a manual check with a real agent before acceptance of the implementation.

## Decided by the maintainer (2026-11-23)
1. **Task and Run move to stage 8** (workflows). Stage 5 is the gate, `sessions.approval_id` traceability and the concurrency limit. This amends the stage list of ADR 0022 and `AGENTS.md` once this ADR is accepted.
2. **Preset launches keep today's behaviour** through the narrow **app** scope (preset launches only, exact executable path).
3. **Custom agents may be approved for a workspace**, chosen by the user and bound to the full fingerprint (path, arguments, environment), so any edit asks again. Otherwise they are reviewed on every launch, as today.
4. **Concurrency limit: default 4, range 1 to 16, counting running agent sessions only.**

## Implementation log (2026-11-23)
Implemented on branch `feat/approval-gate`: `crates/core/src/approval` (gate, migration 4), `agents` (`launch_action`, `migrate_legacy_approvals`), `session` (concurrency limit, `approval_id`), `settings.agents`, Tauri `approval_*`, `agent_authorize` and `agent_launch`, and the launcher, Security panel and Agents setting in the UI.

Differences from, or additions to, the design above:
- A fourth grant state, `expired`, for a once grant or request that timed out unused.
- The gate gained `preview` (would this be allowed, without spending a once grant), `has_app_grant` and `import_grant` (used only to convert old preset approvals). `agent_authorize` previews; `agent_launch` authorizes and spends.
- `agent_launch` refuses at the agent limit **before** authorizing, so a one-time approval is not lost to a launch that could not have started; `SessionManager::start` checks again atomically.
- Editing or deleting a custom agent does not revoke its grants; they stop matching (different fingerprint) and stay listed in Security until revoked.
- The Task and Run model was not built, as decided (moves to stage 8).

Findings while implementing:
- **React StrictMode launched an agent twice** in development (the pane effect runs twice). That used to be a harmless double spawn; with a one-time approval it spent the approval on the first launch and failed the second. Found by the smoke harness, not by unit tests. A pane now starts its session once per component instance.
- `mergeSettings` silently dropped any settings section it did not list; it now carries `agents`.
- The check that `--version` is not run on an unapproved executable was a comment, not a test; it is now enforced by a marker file the mock executable writes when run.

**Evidence from the running app** (sandbox data folder, dev build): an unapproved launch is refused and starts nothing; an agent starts after "allow once"; the database then holds exactly one approval (once, consumed) and the session row links to it; no argument text reaches the approvals table; a missing agent fails with a clear message; at a limit of one, a second agent is refused, its one-time approval is **not** spent, a plain terminal still opens, and after the slot is freed the same agent starts with that approval; two page reloads reattach to the same session with a replayed buffer identical to the one that was on screen.

**Not verified:**
1. The review dialog and the Security approvals panel were exercised through their commands and the database, not looked at; they need a visual pass on macOS.
2. The conversion of old preset approvals is covered by a unit test only, not by upgrading a real data folder.
3. No real agent CLI (Claude Code, Codex, OpenCode) was launched through the gate; only the mock agent was.
4. The macOS-only code from ADR 0026 and everything here that is not macOS-gated were linted on macOS only; Linux and Windows are left to CI.
5. A binary replaced at the same path stays approved (the fingerprint does not hash the executable); this is stated in the review text's design, not shown to the user.

**Still pending the maintainer:** marking this ADR Accepted, after which the stage list in ADR 0022 and `AGENTS.md` is amended (Task/Run moves to stage 8; stage 5 is the gate) and PRD gains requirements for the gate and the limit.
