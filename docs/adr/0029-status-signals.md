# ADR 0029 — Verified status signals per agent, Claude Code first (orchestration stage 7)
**Status:** Accepted (2026-11-23, maintainer; the scope, installation, display, spike and opt-in questions were answered, see the end). **Design and spike results; product code follows in the implementation commits.** Implements stage 7 of ADR 0022 (§4.7). Follows ADR 0011 and AGENTS.md: "do not claim an agent is waiting for approval or completed unless a reliable adapter signal supports it".

## Context
Today the sidebar can say only what the **process** is doing (running, exited). It cannot say that an agent is waiting for the user, because nothing reliable tells Naurix so. Guessing from output (quiet timers, regexes on terminal text) is forbidden (ADR 0022 §1.5), and reading terminal output for it would also break the metadata-first rule.

What exists (from the code and `docs/AGENT_ADAPTERS.md`, 2026-11-23): `Feature::StatusSignals` is **Documented only** for Claude Code, Codex and OpenCode, and unsupported for Gemini CLI and Antigravity CLI. The `AgentAdapter` trait deliberately has no status behaviour yet (ADR 0025 §5). Only Claude Code (2.1.329) and OpenCode (1.18.10) are installed on the development machine, so only they can be verified here.

The product goal is **many agents**. This stage therefore builds one small seam that any adapter can later feed, and verifies it with one agent only.

## Spike results (run 2026-11-23, Claude Code 2.1.329, sandbox folder, `claude -p`, two short calls)
Hooks were injected **per launch** with `--settings <file>`; the user's own settings were not touched. Each hook command appended its stdin to a file.

Verified:
1. `--settings` hooks fire: `SessionStart`, `UserPromptSubmit`, `PermissionRequest` and `Stop` all arrived.
2. They are **merged, not replaced**: a project `.claude/settings.json` hook for `Stop` also ran in the same session.
3. `PermissionRequest` fires when a tool needs approval (here a `Bash` command in a non-interactive run), before the tool is refused or allowed. Its payload has `tool_name`, `tool_input` and `permission_suggestions`.
4. `Stop` fires at the end of a turn (`stop_hook_active`, `background_tasks` and others in the payload).

**Privacy finding that shapes the design:** the payloads carry content. `UserPromptSubmit` has the **prompt**, `Stop` has `last_assistant_message`, `PermissionRequest` has the **tool input** (a command line). A hook that saves its stdin would store exactly what the product rules forbid. The design below never reads stdin.

Not verified (must be checked in the implementation, in the interactive TUI, before any claim):
- The same events in an interactive session (the spike used `-p`), including the folder-trust dialog.
- `Notification` with `idle_prompt` (needs an idle interactive session) and `permission_prompt`.
- What clears "waiting for approval" after the user answers: `PostToolUse`, `PostToolUseFailure` and `PermissionDenied` are candidates, none tested.
- Hook failure and timeout behaviour, and whether a slow hook slows the agent.
- Windows (the hook command is a shell command) and Linux.

## Decision

### 1. One seam, one agent
Core gets a `signals` module with a normalised, closed vocabulary and nothing else:

`AgentSignal { session_id, kind, at }`, where `kind` is one of `Working`, `AwaitingApproval`, `Idle`, `TurnDone`.

An adapter that has a **Verified** `StatusSignals` capability says how its agent produces these (`AgentAdapter` gains a status method, added only now that a capability is verified, per ADR 0025 §5). Claude Code is the only adapter that does so in this stage. Codex (hooks), OpenCode (`/api/event`) and others join later, each after its own verification, and each maps to the same four kinds. An agent without the capability shows process state only, as today.

### 2. Claude Code: injected per launch, nothing written to user config (maintainer decision 2)
- When the capability is enabled, Naurix adds `--settings <inline JSON>` to the launch arguments. The JSON registers hooks for `UserPromptSubmit`, `PermissionRequest`, `Notification` (matcher `idle_prompt`) and `Stop`, and whatever clearing event step 4 of the implementation verifies. Hooks of the user's own settings keep running (merge, verified above).
- **No hook reads stdin.** Each hook command is a fixed one-word writer, for example `printf 'permission_request\n' >> '<spool>'`, so the prompt, answer and tool input are never read, written or logged by Naurix. The word, not the payload, is the signal.
- `~/.claude/settings.json` and project settings are **never modified**; there is nothing to back up or roll back. Running `claude` outside Naurix is unaffected.
- The extra argument is part of the launch the user reviews. The launch fingerprint (ADR 0027) covers the final argument list, so turning the capability on or off asks for approval again; this is intended.
- The capability is **off by default per agent** and switched on in Settings once the user has seen what it does (AGENTS.md: structured capabilities are opt-in).

### 3. Transport: a per-session spool file, no new process and no network
- One append-only file per session under the Naurix data directory (`<data>/signals/<session id>`), created by Core with owner-only permissions (through `platform`), deleted when the session is closed or on the next start.
- Core follows the file from the session's start, accepts only the four known words per line, ignores everything else, and caps the bytes read (an oversize file stops being followed and the agent shows no status).
- The session id in the file name is Naurix's own id, embedded in the command at launch; Claude's `session_id` is not needed or stored.
- No socket, port or helper binary. A hook that fails writes nothing, which shows as "no signal", never as a wrong state.
- Trust boundary, stated plainly: any program running as the same user, including a hook in a project's settings, could write to the file. Signals are **agent-reported and unauthenticated**; they move a badge, never an approval (ADR 0027 is unaffected and never reads them).

### 4. State and display (maintainer decision 3)
- The latest signal and its time live **in memory** in `SessionManager`, delivered over the event bus (metadata only, lossy, latest wins). They are **not persisted**: a status that survives a restart would be false.
- Shown states: **Waiting for approval**, **Waiting for input** (idle), **Turn finished**, **Working**. With no signal the session shows only process state, as now.
- Until the transition out of "Waiting for approval" is verified (above), the badge reads as the **last signal with its age** ("asked for approval 40 s ago"), not as a live claim.
- No message text, prompt, tool name or command is received, stored or shown. No desktop notification is added in this stage.

### 5. Verification level
`Feature::StatusSignals` for Claude Code moves from Documented to **Verified** only for what the spike proved (events 1–4 above, in `-p`, version 2.1.329). The interactive items are added to the table only after the implementation runs them; the version is recorded with each.

## Consequences
- Core gains `signals` (vocabulary, spool follower, state per session) and a status method on the adapter trait; the Claude adapter builds the `--settings` argument. Tauri gains one event and the launch path passes the extra argument. The UI gains a badge and a Settings toggle.
- Tests required: only the four words are accepted and everything else is ignored; an oversize or unreadable spool never crashes or shows a state; hook commands contain no stdin read (asserted on the generated JSON); the spool is owner-only and removed on close; a fake agent that writes words drives the state machine; the argument is part of the launch fingerprint; with the capability off, the launch is byte-identical to today.
- Risk: a hook adds work to every tool call. Mitigation: a single `printf` append, and the overhead is measured in the interactive run before the feature is enabled.
- Risk: Claude Code changes hook names or payloads. Mitigation: the capability is tied to a recorded version and fails to "no signal".

## Decided by the maintainer (2026-11-23)
1. **Scope: Claude Code first**, because it is the only agent that can be verified here, but built as a seam for **multiple agents** (§1).
2. **Installation: per launch with `--settings`**, no writes to user config.
3. **Display: only the verified states**, no message text.
4. **Spike: real calls allowed**; two small `claude -p` runs were made, results above.
5. **The capability is opt-in per agent in Settings**, off by default.

## Still open
- Whether a desktop notification on "Waiting for approval" is wanted in a later stage; it is not part of this one.
