# ADR 0014 — Adapter facts carry a verification level (M6)
**Status:** Accepted

**Context:** Agents document different config formats, move them between versions, and sometimes disagree with the binary on the user's machine. Concretely, while researching M6: OpenCode's current docs describe an `mcp.servers.<name>` shape and "project config by default", but the installed OpenCode 1.18.10 wrote a flat `mcp.<name>` shape into the *global* file. Gemini CLI was retired for most individual accounts on 2026-06-18 in favour of Antigravity CLI (`agy`). Guessing from memory would have produced wrong writes into users' configuration.

**Decision**
1. Every capability of every agent (launch, MCP, skills, status signals, resume) is recorded in code (`agents/adapter.rs`) and in `docs/AGENT_ADAPTERS.md` with one of three levels: **Verified** (confirmed against the real CLI via a committed fixture, or by Naurix's tests), **DocumentedOnly** (official page only), **Unsupported**. **Naurix acts only on Verified.** `AdapterInfo::acts_on` is the single gate later milestones must use.
2. Fixtures are produced by running the real CLI in a throwaway `HOME`/project (never the user's real configuration) and are committed with the exact commands and versions. Tests parse them to guard the claimed shapes, and fail if a "Verified" fact lacks evidence or a recorded version.
3. Config presence checks (`detect_presence`) report **existence only**; the contents of a user's config are not read in M6.
4. `--version` is run only for executables the user has already reviewed and approved (it executes the program), with argument array, null stdin, a 5 s timeout and kill-on-timeout.
5. Status stays **process state only** (ADR 0011). The documented richer signals (Claude/Codex hooks, OpenCode server events) each require changing user configuration or attaching a server, so each is a separate, explicitly approved feature, not part of M6.
6. Added an **Antigravity CLI** preset (`agy`) because the requested Gemini CLI no longer works for most individuals; Gemini CLI stays, with a visible notice. Capability list and notices appear in the launcher ("What Naurix can do with this agent").

**Consequences:** M7 can implement MCP sync only for Claude Code (project `.mcp.json`) and OpenCode (flat shape, existing file) today; the other agents move from *documented* to *verified* only when someone can run them. The matrix needs re-capture when agents update.

**Addendum (2026-11-19, ADR 0016):** a fourth level, *Documented, accepted*, records an explicit maintainer decision to act on a documented but unverified fact **only through a reversible symlink**. It is used for skills folders and nowhere else. `acts_on` is true for it; the UI labels it "Link only, not verified".
