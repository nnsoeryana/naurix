# Naurix — Product Vision

**Status:** Draft v1.1 (post-MVP orchestration direction added by ADR 0022)  
**Product:** Naurix  
**Platform strategy:** macOS Intel (x86_64) and Apple Silicon (arm64), both first-class  
**License:** Apache License 2.0

## 1. Vision

Naurix is an open-source, terminal-first desktop application for developers who work with multiple AI coding agents.

It combines the speed and flexibility of a traditional terminal with a unified workspace for launching, monitoring, and managing AI coding agents such as Claude Code, Codex, Gemini CLI, OpenCode, and custom CLI tools.

**One terminal. Multiple agents. Full control.**

Naurix does not aim to replace the shell, IDE, or the agents themselves. It provides a better environment in which developers can use them together.

**Direction after the MVP (ADR 0022):** Naurix grows into a provider-agnostic orchestration workspace. Several agents can work at once through adapters. The user approves every agent action through one approval gate. Naurix owns opt-in project memory. Workflows can be saved and reused. This is additive: the terminal stays first-class, and nothing here is needed to use it.

## 2. Problem

Developers increasingly run several coding agents and ordinary development processes at the same time. Their work is fragmented across terminal windows, tabs, projects, and tools.

Common problems:
- No unified view of agents across projects.
- Difficult to distinguish running, waiting, completed, and failed sessions.
- Context is lost when windows or application sessions are closed.
- Agent output, shell output, Git changes, and task state are disconnected.
- Each agent has different capabilities and interaction conventions.
- Existing terminal workflows must remain available; AI features should not impose a new workflow.

## 3. Product principles

1. **Terminal first:** a real shell is a first-class feature, not an embedded afterthought.
2. **AI-native, not AI-only:** agent management is built in, but normal terminal usage remains excellent.
3. **User control:** users explicitly launch, stop, approve, and inspect work. No hidden autonomous actions. Every agent action and workflow step passes one central approval gate; approvals are scoped (once, session, run, workspace), bound to what was reviewed, revocable, and never "approve everything".
4. **Local-first:** projects, process execution, and session metadata remain on the user's machine.
5. **Adapter-based:** support multiple agents without coupling the application to one vendor.
6. **Progressive enhancement:** use structured agent protocols when available; retain PTY/CLI compatibility as a universal fallback.
7. **Transparent state:** show what is running, what changed, and what requires attention.
8. **Portable core:** target macOS only, but keep core domain logic free of needless macOS-specific coupling.
9. **Lightweight:** performance is a differentiator and is measured, not asserted (ADR 0006).
10. **Agent-agnostic and capability-honest (ADR 0022):** PTY launch is the baseline for every agent. Other capabilities (structured execution, status, resume) are used only after each is verified for that agent and version. A verified MCP capability does not imply any other.
11. **Runtime separation:** Terminal Runtime (PTYs, processes) is separate from Agent Runtime (adapters, runs, approvals). Core owns lifecycle and logic; Tauri is only the bridge.
12. **Explicit, opt-in memory:** project memory is notes the user writes or approves, per workspace. Prompts, transcripts and terminal output are never saved automatically, and secrets are never stored.
13. **Sessions outlive the view:** sessions survive UI reloads and window close while Naurix is running. On quit they are terminated gracefully. There is no background daemon.

## 4. Target users

### Primary
- Individual software developers using AI coding agents daily.
- Power users who work across multiple repositories and parallel tasks.
- Developers who prefer keyboard-driven terminal workflows.

### Secondary
- Engineering leads reviewing parallel agent work.
- Open-source contributors building custom agent adapters.
- Teams that want a local desktop control plane for coding agents.

## 5. Jobs to be done

- When I start work, I want to open a project workspace and restore my useful sessions.
- When I need an agent, I want to launch it in the correct project directory with the right environment.
- When several agents are working, I want to understand their state without checking every terminal.
- When an agent needs input, I want a clear notification and a direct way to return to it.
- When an agent changes code, I want to inspect the Git diff before accepting or continuing.
- When I close the application, I want session metadata to survive without pretending that every process can always be restored.
- When I reload the UI or close the window, I want my running sessions to keep going.
- When several agents work in parallel, I want to approve what each may do, with a scope I choose, and revoke it later.
- When a project has context worth keeping, I want to write it down once, search it, export it, and delete it, without Naurix recording my sessions.
- When I repeat a multi-step job, I want to reuse it as a workflow that still asks for my approval.

## 6. Product positioning

Naurix is a terminal emulator and local agent workspace for developers who want one place to work with traditional shells and multiple AI coding CLIs.

It is not:
- A hosted AI coding service.
- A replacement for Claude Code, Codex, Gemini CLI, or OpenCode.
- An IDE intended to replace VS Code or JetBrains.
- An autonomous agent that silently edits code in the background.
- A cloud synchronization service in the MVP.

## 7. Differentiators

- Unified project and agent session navigation.
- Terminal-first interaction with an agent-aware layer.
- Hybrid execution: PTY-compatible CLI launch plus structured adapters where supported.
- Metadata-first persistence and explicit process lifecycle.
- Git-aware inspection alongside terminal sessions.
- Open adapter contract for community integrations.

## 8. MVP definition of success

The MVP is successful when a developer can:
1. Install Naurix on macOS.
2. Create and reopen a project workspace.
3. Run a normal interactive shell with reliable keyboard, resize, color, and terminal behavior.
4. Launch supported agent CLIs in a project.
5. See agent sessions in a unified list and distinguish their basic lifecycle states.
6. Return to a live session while the app remains running.
7. Reopen the app and recover workspace/session metadata with honest indication of sessions that cannot be reattached.
8. Inspect basic Git status.
9. Install and configure MCP integrations and agent skills from Settings without hand-editing config files.
10. Use the app without signing in or configuring a hosted account.

## 9. Product boundaries and non-goals for MVP

- No built-in model inference or model-provider API key management (MCP server credentials live in the Keychain).
- No public MCP or skills marketplace.
- No unapproved orchestration. Multi-agent runs, handoff and workflows are post-MVP (ADR 0022) and always user-approved; none exist in the current build.
- No cloud execution, remote machines, or session synchronization.
- No background daemon or supervisor.
- No automatic capture of prompts, transcripts or terminal output as memory.
- No full IDE, editor, debugger, or file explorer replacement.
- No guarantee that arbitrary CLI processes can be resumed after app restart.
- No automatic interpretation of every agent's internal state. Unknown state must be displayed as `Unknown` or `Running`, not guessed.
- No permanent full terminal transcript storage by default.

## 10. Product experience

### Primary information architecture
- **Workspaces:** project folders and their sessions.
- **Sessions:** terminal or agent process instances.
- **Main canvas:** terminal tabs and split panes.
- **Inspector:** session details, Git status/diff, process metadata.
- **Command palette:** keyboard-first navigation and actions.
- **Status bar:** active workspace, shell, Git branch, and runtime status.

### Core user journey
1. Create workspace from a local directory.
2. Open a terminal session.
3. Launch an agent using a detected CLI or configured custom command.
4. Monitor the session from the agent/session list.
5. Focus the session when it needs attention.
6. Inspect project Git changes.
7. Close or terminate sessions explicitly.
8. Reopen the app and recover workspace metadata.

## 11. Open-source strategy

- License: Apache License 2.0.
- Public repository with issues, discussions, roadmap, contribution guide, and security policy.
- Keep provider-specific integrations behind documented adapter interfaces.
- No telemetry by default. If telemetry is ever introduced, it must be opt-in, documented, and privacy-preserving.
- Do not collect terminal content, source code, prompts, or environment variables.
- Community adapters must declare capabilities and permissions clearly.

## 12. Risks

| Risk | Mitigation |
|---|---|
| Terminal compatibility is harder than expected | Validate PTY + xterm.js early; maintain a terminal conformance checklist |
| Agent CLIs change frequently | Isolate integrations in adapters; support generic CLI fallback |
| Agent state is not consistently observable | Use confidence-aware state model; never infer unsupported states as facts |
| Process lifecycle differs by OS | Abstract process management; define macOS behavior first |
| App becomes too broad | Enforce MVP non-goals and milestone gates |
| Sensitive terminal data is persisted accidentally | Metadata allowlist, no transcript persistence by default, redact secrets from logs |

## 13. Product decision log

| Decision | Current choice | Rationale |
|---|---|---|
| Product direction | Terminal-first | Preserve shell as the primary experience |
| Platform | macOS first | Focus initial implementation and validation |
| Desktop framework | Tauri 2 | Rust core with web-based UI |
| Frontend | React + TypeScript | Familiar component ecosystem |
| Terminal rendering | xterm.js MVP; evaluate libghostty | Ship quickly while validating native alternative |
| Agent execution | Hybrid | PTY fallback plus structured integrations |
| Persistence | Metadata-first | Useful recovery without storing terminal content |
| License | Apache-2.0 | Permissive open-source license with patent grant |
| Platform | macOS x86_64 + arm64 | ADR 0001 |
| MCP / Skills | Central registry + sync adapters | ADR 0003 |
| Orchestration | In scope post-MVP, staged, approval-gated | ADR 0022 |
| Session lifecycle | Survive reload and window close; graceful terminate on quit; no daemon | ADR 0022 |
| Project memory | SQLite + FTS5, opt-in per workspace, explicit notes, Markdown/JSON export, never auto-committed | ADR 0022 |
| Agent execution | PTY baseline; structured execution only per verified capability | ADR 0022, 0014 |
