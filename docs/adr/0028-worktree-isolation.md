# ADR 0028 — Opt-in Git worktree isolation for agents (orchestration stage 6)
**Status:** Accepted (2026-11-23, maintainer). **Implemented; see the implementation log at the end for what is not yet verified.** Implements stage 6 of ADR 0022 (§4.6) and the P2 requirement GIT-006. It makes the first **writing** use of Git in Naurix, which amends ADR 0005 and ADR 0017 ("nothing writes") for exactly the two operations below.

## Context
Two agents working in the same folder can overwrite each other's files, and nothing tells the user. A Git worktree gives each agent its own checkout of the same repository, on its own branch, in its own folder, so work stays separate and is merged with ordinary Git afterwards.

What exists today (from the code, 2026-11-23):
- Git access is **read-only** (`status`, `diff`, `worktree list`, `config --local --list`) through one hardened runner, `crates/core/src/git/run.rs`.
- That runner already neutralises the vectors that matter for a write: `core.hooksPath=/dev/null` (so a repository's `post-checkout` hook cannot run), `core.fsmonitor=false`, `diff.external=`, `credential.helper=`, `--no-optional-locks`, a scrubbed environment, no terminal prompts, bounded output and a timeout.
- A repository whose **local** config defines `filter.*.clean/smudge/process` or `include.path` is **Blocked** until the user trusts it (ADR 0017 §3). A checkout runs smudge filters, so this check must also gate worktree creation.
- The Git view already **lists** worktrees (GIT-005). `SessionLaunch.cwd` already lets the backend start a session somewhere other than the workspace root, and the webview can never set it.
- The approval gate (ADR 0027) exists; ADR 0022 fixed that Git mutations are approved **once** only.

## Decision

### 1. Two new approval classes, once only
`CreateWorktree` and `RemoveWorktree`, each with maximum scope **once**. Neither can be granted for a workspace or app-wide, and neither can ever run in the background or as a side effect of something else. The review shows the exact Git command line, built by the backend.

### 2. Creating a worktree
- **Opt-in per launch.** The launcher offers "Run in a separate worktree", **off by default**, shown only when the workspace is a Git repository that is not Blocked. Existing behaviour does not change.
- Command (argument array, no shell, through the existing hardened runner with a longer timeout): `git worktree add -b <branch> <path> <base>`. `<base>` is the commit `HEAD` points to at review time (resolved and shown as a short hash). A repository with no commits is refused with a clear message.
- **Branch name:** `naurix/<agent-id>-<yyyymmdd-hhmm>`, with a numeric suffix if it exists. Characters outside `[a-z0-9._-/]` are replaced, and the result is validated with `git check-ref-format --branch` before anything is created.
- **Location:** Naurix's own data folder, `<app data>/worktrees/<workspace id>/<branch slug>` (maintainer decision 1), never a path the webview supplies. The path must not exist.
- **Order, so nothing irreversible is wasted:** the agent limit is checked and the launch approval is authorized first; the worktree is created immediately before the session starts. If starting the session then fails, the worktree is **not** removed automatically (removal is a mutation that needs its own approval); the user is told where it is and it appears in the list for cleanup.
- Fingerprint: class, repository root, branch, path and base commit. A change to any asks again.
- Hardening carries over from ADR 0017, and hooks stay disabled. This means a repository's own `post-checkout`, `commit` and similar hooks do **not** run during creation; that is stated in the review. Smudge filters from the user's own (global) Git configuration still run, as they do for any checkout (for example Git LFS).
- **Not copied into the worktree, ever:** untracked or ignored files (`node_modules`, `.env`, build output). Copying them could copy secrets, so the user or the agent sets those up. Submodules are not initialised either. Both are listed as known limits in the review text.

### 3. Removing a worktree
- Only worktrees **recorded as created by Naurix** can be removed by Naurix; others are shown but offered no remove. The webview names a worktree **id**, never a path.
- Command: `git worktree remove <path>` with **no `--force`**. Git refuses when the worktree has modified or untracked files or is locked; Naurix passes that refusal on and removes nothing.
- It is also refused while a running session uses that worktree.
- The review includes a read-only summary of the worktree's status, and the fingerprint covers its `HEAD` and that status, so a change between review and action asks again.
- **The branch is kept.** Commits made by an agent live on that branch; deleting it is a separate action that is not part of this stage.

### 4. Data model
Migration 5: table `worktrees` (`id`, `workspace_id` nullable with `ON DELETE SET NULL`, `repo_root`, `path`, `branch`, `base_commit`, `created_at`) and `sessions.worktree_id`. Removing a workspace never deletes project files, so it does not delete worktree folders either: the row stays (the workspace link is cleared) and the worktree is listed under Settings, Security so it can be cleaned up. Metadata only; nothing from inside a worktree is stored.

### 5. What the UI shows
- Launcher: the opt-in checkbox, and a **warning (never a forced change)** when another agent is already running in the same folder, suggesting a worktree.
- Review: both the worktree command and the agent command, each with its own approval choices (the worktree one is "once" only).
- Git view, Worktrees: a "created by Naurix" mark, **Open terminal here** and **Remove** (disabled with the reason when dirty, locked or in use).
- Sidebar: an agent running in a worktree shows its branch.
- Settings, Security: worktrees whose workspace is gone, with Remove.
- A worktree is **not** added as a workspace (maintainer decision 4).

### 6. Limits stated up front
- Cross-platform: path handling goes through `platform`; Windows and Linux are unvalidated as elsewhere (ADR 0021).
- A very large repository may need longer than the timeout to check out; the failure is reported as is and leaves nothing half-managed (Git cleans up a failed `worktree add`; the record is only written after success).
- Naurix does not merge, rebase, push or delete branches in this stage.

## Consequences
- ADR 0005 and 0017 gain this single exception to "nothing writes"; every other Git call stays read-only.
- Core gains `git::worktree` (plan, create, plan-remove, remove), two approval classes, migration 5. Tauri gains create/remove/list-managed commands and extends `agent_authorize`/`agent_launch` with the opt-in. The UI changes listed above.
- Tests required: creation against a real repository (branch and folder exist, base is right, repo `HEAD` and working tree unchanged); a hostile repository with a `post-checkout` hook and a malicious `core.fsmonitor` (marker files must **not** appear, with a naive `git worktree add` shown to create them so the test is meaningful); a repository with a local clean/smudge filter is refused until trusted; no commits, bare repository, branch already exists, path exists; removal refuses dirty, locked and in-use worktrees and never passes `--force`; a once approval cannot be reused; a path or id the UI did not get from the backend is rejected; workspace removal leaves the folder and keeps a listed record; launch order (a failed launch leaves a listed worktree, never a silent deletion).
- Risk: Git writing is new for Naurix. Mitigation: once-only approval, the hardened runner, no `--force`, no automatic deletion, and the hostile-repository tests.

## Decided by the maintainer (2026-11-23)
1. **Worktrees live in Naurix's data folder** (`<app data>/worktrees/<workspace id>/<branch slug>`), not beside the repository.
2. **Removing a worktree always keeps its branch.** Deleting a branch is a separate action, not part of this stage.
3. **The launcher option is off by default**, with a warning (a suggestion only, nothing forced) when another agent is already running in the same folder.
4. **A worktree is not a workspace.** It is an attribute of the run, listed under its repository.

## Implementation log (2026-11-23)
Implemented on branch `feat/worktree-isolation`: `crates/core/src/git/worktree.rs` (plan, create, plan-remove, remove, approval actions, records), migration 5, the `create_worktree` and `remove_worktree` approval classes, session linkage, Tauri `worktree_*` commands and the opt-in on `agent_authorize`/`agent_launch`, and the launcher option, review, Git view controls, sidebar badge and Security list in the UI.

Differences from, or additions to, the design above:
- **The creation fingerprint omits the time part of the branch name** (it covers repository, base commit, agent and folder). A name that includes the time would change between the review and the launch, so a one-time approval could never match. The exact name is finalised at creation, and the review says so.
- **The branch is named after the agent's display name**, not its id: `naurix/wt-agent-20261123-0436`, not `naurix/custom-18ea92...`. Found in the running app.
- An action with **no workspace** (a worktree whose workspace was removed) is stored with a NULL workspace; `approvals.workspace_id` is a foreign key, so an empty id would have failed to insert.
- A launch **previews both approvals before spending either**, so a missing approval never wastes the other; the worktree is created last, right before the session. A worktree whose folder was deleted by hand can be **forgotten** (Naurix's own record only, no Git change, so no approval).
- `worktree_support` reuses the planning step to say whether the option can be offered, and why not.

**Evidence from the running app** (sandbox, a real Git repository with a committed history and ignored `.env` and `node_modules`): a launch with nothing approved is refused and creates no worktree; with approvals the worktree is created, the agent starts in it and its branch appears in the sidebar; a terminal opened in the worktree works in that folder; removal is refused while an agent runs, refused while an untracked file exists, and refused without an approval, then succeeds with a reviewed once-approval. Afterwards the main repository is on `main`, clean, at the same commit; Git lists only the main worktree; the folder is gone; **the agent's branch is kept**; sessions are linked to the worktree and the agent session to its approval; the create and remove approvals are each `consumed` exactly once. In Core, a hostile repository (its own `post-checkout` hook, and one named by its config) is shown to fire for a plain `git worktree add` and not for Naurix's, and a repository with a local clean filter is refused until trusted.

**Not verified:**
1. The launcher checkbox, the same-folder warning, the two-part review, the Git view buttons and the Security list of left-behind worktrees were exercised through their commands, not looked at; they need a visual pass on macOS.
2. Removing a worktree whose workspace is gone is covered in Core only, not in the app.
3. No real agent CLI was started in a worktree, only the mock agent.
4. Smudge filters from the user's global Git configuration (for example Git LFS) were not exercised; a repository needing LFS without `git-lfs` installed would fail the checkout and report Git's message.
5. A very large repository may exceed the 120 second creation limit; submodules are not initialised (stated in the review). Neither was tried.
6. Linux and Windows were not run (paths go through `platform`, as elsewhere).

**Still pending the maintainer:** marking this ADR Accepted, after which PRD records GIT-006 and ORCH-006 as implemented and `AGENTS.md` lists stage 6 as done.
