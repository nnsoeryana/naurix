# ADR 0025 — `AgentAdapter` trait for per-agent behaviour (orchestration stage 3)
**Status:** Proposed (maintainer to accept). Implements stage 3 of ADR 0022. **Behaviour-preserving: no user-visible change.**

**Context:** `agents/adapter.rs` holds per-agent *facts* (a static table with verification levels, ADR 0014). Per-agent *behaviour* is scattered as literal agent-id checks: `mcp/sync.rs` (`target_path`, `shape_of`, a Claude-only note) and `skills/scan.rs` (`compat`, a hard-coded list of five ids). Adding an agent means editing those files. ADR 0022 requires that adding an agent touch only its adapter.

**Decision**
1. Add the `AgentAdapter` trait in `crates/core/src/agents/behavior.rs`, with one implementation per built-in preset (Claude Code, Codex, Gemini CLI, Antigravity CLI, OpenCode). It has exactly the behaviours that exist today:
   - `info()`: the existing `AdapterInfo` facts (unchanged).
   - `mcp_target(workspace_root, home) -> Option<McpTarget>`: the config file Naurix may write and its verified file layout (`McpFileFormat`), plus an optional note shown with the sync plan.
   - `skill_compat(SkillFacts) -> SkillCompat`: compatibility of a skill's front matter with this agent.
2. **Verification gating is not bypassed.** `sync` still calls `info().acts_on(Feature::McpConfig)` before `mcp_target`, so an adapter cannot enable writing by returning a path. A test asserts that an adapter returns an MCP target **exactly when** its McpConfig capability is Verified or accepted.
3. `adapter()` and `all_adapters()` (the facts API and its tests) stay as they are. A separate `adapter_for(id)` / `agent_adapters()` registry exposes the trait objects. A test checks both registries list the same agents.
4. Replace the `match agent_id` code in `mcp/sync.rs` and the id list in `skills/scan.rs` with calls through the trait. Output, error messages, ordering and file contents stay identical. `McpFileFormat` replaces the private `Shape` enum; its variants are named after the agent each layout was captured from, and another agent that uses the same layout reuses the variant.
5. **Deliberately not in the trait yet:** structured events, headless run, status signals, resume, detect, and launch-spec building. No agent has any of the first four Verified (ADR 0022 §1), and launch is still the generic PTY path (`AgentDef`/`plan_launch`, which also serves custom agents). Each is added as its own method when its first capability is verified, together with the `Feature` variant and fixture. This follows AGENTS.md: no speculative abstractions.
6. Custom agents have no adapter; they keep the generic launch only (unchanged).

**Consequences**
- Adding an agent's MCP or skill behaviour touches `behavior.rs` (and the facts table), not sync or scan code.
- Remaining agent-id literals are data: the preset list, the facts table, and tests.
- The skills compatibility list keeps its current display order (Claude, Codex, OpenCode, Antigravity, Gemini) because the trait registry preserves it. The facts registry keeps preset order. Both are guarded by a test, so they cannot silently diverge.
- Implemented 2026-11-21: `agents/behavior.rs` (trait, five adapters, `adapter_for`/`agent_adapters`, 3 unit tests), `mcp/sync.rs` (`target`, notes from the adapter), `skills/scan.rs` (`compat` iterates the registry). Verified with `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` and `cargo test --workspace`, including the unchanged MCP sync and skills suites. The app was not launched interactively.
- Risk: a subtle change in MCP file location or rendering. Mitigation: the existing MCP sync fixtures and skills tests must pass unchanged.
