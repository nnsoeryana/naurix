# ADR 0007 — Scope clarifications vs. the initial vision docs
**Status:** Accepted
**Context:** The initial docs predate the MCP and Skills requirements and conflict with them in places.
**Decision:**
1. Non-goal reworded: "no model-provider API keys / inference". MCP server credentials are in scope (ADR 0004).
2. MCP Manager and Agent Skills Manager are P0 MVP features; no public marketplace — a small static catalog plus manual configuration.
3. Git: status, branch, changed files, open-in-terminal are P0; per-file diff is P1 (it was P0). No Git client.
4. Launch confirmation: full command review is required for custom agents and the first launch of a preset; subsequent preset launches do not re-prompt.
5. Styling: Tailwind CSS v4 (via `@tailwindcss/vite`), chosen by the maintainer in M1. Node 26 is used for development; `engines` requires >=22.
6. Settings sections: General, Terminal, AI Agents, MCP Integrations, Agent Skills, Performance, Security.
