# ADR 0028 — Opt-in Git worktree isolation for agents (orchestration stage 6)
**Status:** Proposed (maintainer to accept; the four decisions were answered on 2026-11-23, see the end). **Design only; no code.** Implements stage 6 of ADR 0022 (§4.6) and the P2 requirement GIT-006. It makes the first **writing** use of Git in Naurix, which amends ADR 0005 and ADR 0017 ("nothing writes") for exactly the two operations below.

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
