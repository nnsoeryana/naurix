# ADR 0022 — Orchestration workspace scope, session lifecycle and shared project memory
**Status:** Accepted (2026-11-21, maintainer). Documentation only: **no feature is implemented by this ADR.** Supersedes the orchestration non-goals in ADR 0007 context, PRODUCT_VISION §9, PRD §3 and AGENTS.md "Scope control". Replaces the "a page reload terminates all sessions" behaviour, which is currently only in code (`src-tauri/src/lib.rs`, `on_page_load`) and not recorded in an earlier ADR (see §2). ADR 0008's termination ladder, ADR 0009's terminal byte path and ADR 0010's tabs/panes model are unchanged.

**Context:** Naurix was scoped as a terminal-first launcher and manager for AI coding CLIs, and several documents excluded orchestration. On 2026-11-21 the maintainer asked to evolve Naurix into a provider-agnostic AI Agent Orchestration Workspace with five principles: agent-agnostic, multi-agent, human-controlled, shared memory, reusable workflows. A read-only audit the same day found the core crate is a good base, with gaps in session ownership (`start_session` lives in `src-tauri`, sessions die on webview reload), events (no in-core event bus), adapter behaviour (adapters are a static facts table, with `match agent_id` literals in MCP/skills code), and identity (runtime `SessionId` versus database id). It also found that "shared memory" collides with the metadata-first invariant, which this ADR resolves explicitly.

## Decision

### 1. Agent execution
1. **PTY-based execution is the universal baseline.** Every agent, including custom CLIs, can run through the generic PTY adapter (ADR 0011). Nothing in this ADR removes or weakens it.
2. **Verification is per capability, never inferred.** Claude Code and OpenCode are Verified for **MCP configuration only** (`docs/AGENT_ADAPTERS.md`). That says nothing about headless or structured execution, status signals, or resume. Each of those is a separate capability that needs its own `Feature` entry, its own evidence (a committed fixture from running the real CLI, or Naurix's own tests) and a recorded agent version before Naurix acts on it (ADR 0014).
3. **Structured execution is opt-in and gated.** It is enabled for an agent only after that exact capability is Verified (or explicitly Documented-accepted by the maintainer for a reversible action). Until then that agent stays PTY-only and its status stays Running or Unknown.
4. **Other agents integrate through adapters.** Codex, Gemini CLI, Antigravity CLI and any future agent are added as adapters. They begin at PTY level and gain capabilities one at a time as they are verified. Adding an agent must not require edits outside its adapter (today it does: see the audit).
5. **Agent state is never guessed.** "Waiting for approval", "working" and "finished" appear only from a verified adapter signal (ADR 0011, AGENTS.md). Heuristics such as output-quiet timers must not be shown as agent state and must not drive task or workflow transitions.

### 2. Session lifecycle
1. **Sessions survive UI reloads.** Reloading the webview must not terminate sessions. The webview detaches and later reattaches to a session that Core owns. This replaces the `on_page_load → shutdown()` policy.
2. **Sessions keep running when the application window closes**, provided the Naurix application process stays alive. Closing the window is not quitting. Where a platform quits the process when the last window closes, that platform's behaviour must be defined and tested before this is claimed there; today only macOS is a supported platform (ADR 0001, ADR 0021).
3. **On application quit, agent and terminal processes are terminated gracefully** (existing ladder: SIGHUP, SIGTERM, SIGKILL after grace; Windows tree kill), as `shutdown_all` does today. Quit is explicit and tested, never implicit OS behaviour.
4. **No separate background daemon or supervisor** in this phase. Process ownership stays in the single Naurix process. A daemon may be proposed later by a new ADR.
5. **Restart is unchanged (DATA-004):** after the app process exits, sessions recorded as running are marked `interrupted` and are not reattachable. Only UI reload and window close preserve sessions.
6. Any buffer kept so a reattached view can repaint is **in memory only**, bounded, and never persisted. Its size and format are decided in the session-ownership ADR.

### 3. Shared project memory
1. **Storage is SQLite** (the existing database and migration mechanism), with **FTS5** for full-text retrieval. Before implementation, confirm FTS5 is compiled into the `rusqlite` build already in use (bundled SQLite) so no new dependency is needed. Memory text and its FTS index are plain local data, so deleting a memory entry must delete its FTS rows as well (tested).
2. **Explicit and opt-in per workspace.** Memory is off until the user enables it for that workspace. It consists of notes the user writes or approves.
3. **Never automatic.** Naurix never persists prompts, transcripts, terminal output, environment values or source content as memory, and never captures them in the background. Automatic summarisation of agent output is out of scope and would need its own ADR (redaction, retention, deletion).
4. **Never secrets.** `looks_like_credential` runs on save and rejects suspected secrets. Secrets stay in the OS credential store only (ADR 0004, ADR 0019).
5. **The user controls it:** view, edit, delete (single entry or the whole workspace's memory), and export. Removing a workspace deletes its memory.
6. **Export formats are Markdown and JSON**, written only to a location the user chooses.
7. **Never committed to Git automatically.** Naurix does not write memory or exports into a repository, and does not run `git add`/`git commit` for them. The memory database lives in the app data folder, not in a workspace folder. If the user chooses an export destination inside a Git repository, Naurix warns before writing.
8. **Delivery to an agent is never silent.** Memory reaches an agent only through an approved, reversible mechanism (decision 4) such as a file the user approves with dry-run, backup and rollback (ADR 0015/0016 pattern). The mechanism is chosen in the memory ADR.

### 4. Approval gate and runtime separation
1. **One centralized approval gate** governs every agent action and workflow operation: launching an agent, sending automated input, starting a run or workflow step, creating a Git worktree, and writing agent configuration. It generalises the existing review and approval patterns (ADR 0007 §4, ADR 0011 §6, ADR 0015) so there is one place that decides, records and revokes.
2. **Scoped approvals** avoid repeated prompts without giving up control:
   - An approval names an **action class and exact target** and is bound to a **fingerprint** of what was reviewed (as MCP approval is bound to command, args and URL). If the target changes, the approval no longer applies.
   - Scopes, narrowest first: **once**, **this session**, **this run or workflow run**, **this workspace**. The user picks the scope; Naurix never widens it.
   - Each action class has a **maximum scope**. Defaults: Git mutations (including worktree creation) and writes to agent config files are **once** only; launching a previously reviewed agent executable may be granted for the workspace; automated input is limited to the session or run.
   - Approvals are listed in Settings, can be revoked, and are cleared when the workspace is removed. A record holds metadata only (class, target fingerprint, scope, time), never content.
   - There is no "approve everything" scope.
3. **Agent Runtime is separate from Terminal Runtime.**
   - **Terminal Runtime** owns PTYs, process lifecycle, flow control and the byte path to the view (existing `crates/core/src/pty`, ADR 0008/0009). It knows nothing about agents, tasks or memory, and normal shell use depends only on it.
   - **Agent Runtime** sits above it: adapters, agent sessions, runs and tasks, approvals, and verified status signals. It asks the Terminal Runtime to start and stop processes and never reads or stores terminal bytes except through an adapter channel that has been verified.
   - Terminal use must keep working with the Agent Runtime absent or disabled (terminal-first invariant).
4. **Tauri is the bridge, Core owns behaviour.** `src-tauri` validates input, calls Core services, and adapts Core output to Tauri `Channel`s and events. Session start, lifecycle, persistence updates and approval decisions live in `crates/core`. The existing `start_session` and `Terminals` in `src-tauri/src/terminal.rs` are known debt to be moved.
5. **Workflows and tasks are metadata.** Task, Run, Workflow and Approval records hold ids, user-typed names, dependency edges, timestamps and states, not output or prompts.
6. **Isolation by explicit worktree.** Concurrent runs in one repository may request a Git worktree. Creating one is a Git mutation, needs approval at **once** scope, and is never done in the background (AGENTS.md: no silent Git mutation).

### 5. Scope
Orchestration is in scope in the staged form here. **Still out of scope:** cloud execution or sync, remote machines, team collaboration, a full IDE, built-in model inference or provider API-key management, a public marketplace, an autonomous agent that acts without approval, and a background daemon. Terminal-first and "AI never required" remain invariants: a user who never creates a task or workflow sees the existing product.

## Consequences
- PRD, PRODUCT_VISION and AGENTS.md are updated with this ADR so they agree.
- Not covered by this ADR and needing their own ADR before the code lands: **session ownership and reattach** (including the replay buffer and window-close behaviour), **domain events** (an in-core bus, separate from the terminal byte path), **AgentAdapter trait and per-capability gating** (new `Feature` variants), **Task/Run/Workflow data model and migrations**, **approval gate data model**, **structured status signals per agent**, **memory store and delivery**.
- Staged implementation order (each stage leaves the app runnable and existing tests green; no new user-visible orchestration before stage 5): (1) move session start and ownership from `src-tauri` into Core with one stable session id, **preserving existing terminal behaviour and avoiding unrelated refactoring**; (2) event bus; (3) `AgentAdapter` trait with no behaviour change; (4) sessions independent of the webview and window; (5) approval gate and agent concurrency limit (the Task/Run model moved to stage 8, ADR 0027 §6); (6) opt-in worktree isolation; (7) structured-signal spikes per agent; (8) workflows, including Task/Run; (9) memory.
- Stage 4 is the riskiest: it changes the reload shutdown policy and must be covered by process-lifecycle tests (Spike D equivalent) before it is relied on.
- Concurrency and platform claims need a recorded run on real hardware before they appear in docs (ADR 0006, ADR 0021). The existing four-agent benchmark does not cover orchestrated runs.
- Risk: scope growth. Mitigation: stage gates above and the implementation workflow in AGENTS.md.

## Decided here versus left open
Decided: PTY baseline; per-capability verification; session survival across reload and window close; graceful terminate on quit; no daemon; SQLite plus FTS5 memory, opt-in, Markdown/JSON export, no auto-commit; central scoped approval gate; Agent Runtime separate from Terminal Runtime; Core owns lifecycle.
Left to follow-up ADRs: replay buffer size and format; exact approval data model and default scopes beyond those above; memory delivery mechanism; which agent gets a headless or structured capability first (it must be verified first); window-close behaviour on Linux and Windows.
