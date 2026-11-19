# Naurix — Agent adapters: verified capability matrix

**Captured:** 2026-11-19, macOS 14.6 x86_64. Facts come from official documentation *and* from running the CLIs installed on the development machine (Claude Code 2.1.322, OpenCode 1.18.10) in a throwaway `HOME`. Codex CLI, Gemini CLI and Antigravity CLI are **not installed** there, so for them nothing is more than "documented".

## How to read this

| Level | Meaning | Naurix may act on it? |
|---|---|---|
| **Verified** | Confirmed by running the real CLI (fixture in `crates/core/tests/fixtures/agents`) or by Naurix's own tests | Yes |
| **Documented only** | An official page describes it; not confirmed here | **No** |
| **Documented, accepted** | Documented; the maintainer decided Naurix may act on it **only through a reversible symlink** (ADR 0016). Discovery by the agent is not verified | Symlink only |
| **Unsupported** | No basis yet | No |

The same data lives in code (`crates/core/src/agents/adapter.rs`), is shown in the launcher ("What Naurix can do with this agent"), and tests fail if a capability is promoted to *Verified* without evidence or a version recorded. When an agent changes, re-capture its fixture and update both.

## Matrix

| | Claude Code | Codex CLI | Gemini CLI | Antigravity CLI (`agy`) | OpenCode |
|---|---|---|---|---|---|
| **Launch in a terminal** | Verified (generic PTY; real CLI not started by tests) | same | same | same | same |
| **MCP configuration** | **Verified**, project scope only | Documented only | Documented only | Documented only | **Verified** for the flat shape of 1.18.10 |
| **Skills** | Accepted: link only | Accepted: link only | Unsupported (not researched) | Accepted: link only | Accepted: link only |
| **Task status (waiting / done)** | Documented only (hooks, opt-in) | Documented only (hooks) | Unsupported | Unsupported | Documented only (server events) |
| **Resume sessions** | Documented only | Unsupported | Unsupported | Unsupported | Unsupported |
| **Verified with** | 2.1.322 | – | – | – | 1.18.10 |

## Per-agent facts and sources

### Claude Code
- **MCP** ([docs](https://code.claude.com/docs/en/mcp)): scopes *local* and *user* are stored in `~/.claude.json` (a large file that also holds unrelated state), *project* in `.mcp.json` at the project root. **Verified fixture** (`claude mcp add --scope project …`): `{"mcpServers": {"<name>": {"type": "stdio", "command": "…", "args": [...], "env": {...}}}}` and `{"type": "http", "url": "…", "headers": {...}}`. `${VAR}` / `${VAR:-default}` expansion is documented. Project servers prompt for approval in interactive sessions. *Decision:* Naurix will only ever write the **project** `.mcp.json`; it will not edit `~/.claude.json`.
- **Gotcha found by running it:** `--env` is variadic and swallows the next word, so `--env K=v name -- cmd` fails; use `name -e K=v -- cmd`. (Relevant only if Naurix ever shells out; it writes files directly.)
- **Skills** ([docs](https://code.claude.com/docs/en/skills)): `~/.claude/skills/<name>/SKILL.md` (personal) and `.claude/skills/<name>/SKILL.md` (project); frontmatter fields are optional (`name` defaults to the folder, `description` recommended); changes are picked up live; skills may bundle scripts that **Claude** runs; on/off via deleting the folder or `skillOverrides` in settings.
- **Status signals** ([hooks](https://code.claude.com/docs/en/hooks)): events `Notification` (matchers incl. `permission_prompt`, `idle_prompt`), `PermissionRequest`, `Stop`, … configured in `~/.claude/settings.json` or project settings. Needs a hook entry in the user's settings: **not added** without an explicit, reversible opt-in.
- **CLI** ([reference](https://code.claude.com/docs/en/cli-reference)): `--version`, `--continue/-c`, `--resume/-r`, `-p` with `--output-format`, `--mcp-config`, `--strict-mcp-config`, `--settings`.

### OpenAI Codex CLI (docs now served from `learn.chatgpt.com`)
- **Config** ([reference](https://learn.chatgpt.com/docs/config-file/config-reference)): `~/.codex/config.toml`; project `.codex/config.toml` only when the project is trusted.
- **MCP** ([docs](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)): TOML tables `[mcp_servers.<name>]` with `command`, `args`, `env`, `env_vars`, `cwd` (stdio) or `url`, `bearer_token_env_var`, `http_headers` (streamable HTTP), plus `enabled`, `startup_timeout_sec`, `tool_timeout_sec`. CLI: `codex mcp add|list|login` (no `remove` documented). OAuth client secrets are saved in `config.toml`.
- **Skills** ([docs](https://learn.chatgpt.com/docs/build-skills)): `$HOME/.agents/skills`, `$CWD/.agents/skills` (up to the repo root), `/etc/codex/skills`; `SKILL.md` requires `name` and `description`; disable with `[[skills.config]]` (`name` or `path`, `enabled = false`).
- **Status:** `[hooks]` in `config.toml` (events include `PermissionRequest`, `Stop`; needs `features.hooks`). The `notify` setting is "a command that receives a JSON payload", but event names and payload are **not documented on the reference page**.

### Gemini CLI — **retired for most individual accounts**
- Google's announcement ([19 May 2026](https://developers.googleblog.com/en/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)): from **18 June 2026** Gemini CLI stops serving Google AI Pro/Ultra and free Gemini Code Assist for individuals. It "will remain accessible via paid Gemini and Gemini Enterprise Agent Platform API keys", and Gemini Code Assist Standard/Enterprise customers are unchanged.
- **MCP** ([docs](https://geminicli.com/docs/tools/mcp-server/)): `~/.gemini/settings.json` / `.gemini/settings.json`, key `mcpServers` (`command`, `args`, `env`, `cwd`, `url`, `httpUrl`, `headers`, `timeout`, `trust`, `includeTools`, `excludeTools`); `gemini mcp add|list|remove|enable|disable`.
- The preset stays (it is in the product requirements) and the launcher shows a warning.

### Antigravity CLI (`agy`) — Gemini CLI's successor
- Source: [migration guide](https://antigravity.google/docs/cli/gcli-migration). Command `agy`. MCP: `~/.gemini/config/mcp_config.json` (global) and `.agents/mcp_config.json` (workspace), key `mcpServers`; remote servers use **`serverUrl`** (legacy `url`/`httpUrl` must be renamed). Skills: `~/.gemini/skills/`, `~/.gemini/antigravity-cli/skills/`, `.agents/skills/`. Gemini extensions become *plugins* (`agy plugin import gemini`). Session tokens move to the OS keyring.
- Added as a **preset** because the requested "Gemini CLI" no longer works for most individuals. Easy to remove if unwanted.

### OpenCode
- **Config** ([docs](https://opencode.ai/docs/config/)): `~/.config/opencode/opencode.json(c)`, project `opencode.json(c)` or `.opencode/opencode.json(c)` merged upward; managed settings under `/Library/Application Support/opencode`.
- **MCP — docs and the installed binary disagree.** Current docs ([MCP page](https://opencode.ai/docs/mcp-servers/)) show `mcp → servers → <name>` ("V2") with `disabled`, and say `opencode mcp add` writes the project config by default. **Installed 1.18.10 actually wrote** the **flat** `mcp.<name>` shape (`type: "local"`, `command` array, `environment`) into the **global** `~/.config/opencode/opencode.jsonc` (fixture). *Decision:* Naurix acts only when an existing file already uses the verified flat shape; anything else is reported as unsupported rather than guessed.
- **Skills** ([docs](https://opencode.ai/docs/skills/)): `~/.config/opencode/skills`, `.opencode/skills`, and (compat) `~/.claude/skills`, `.claude/skills`, `~/.agents/skills`, `.agents/skills`. Frontmatter is optional; the ID comes from the path.
- **Status** ([server docs](https://opencode.ai/docs/server/)): `opencode serve` exposes an experimental `GET /api/event` with `session.idle`, `session.status`, `permission.asked`/`permission.replied`. The installed CLI has `serve`, `attach` and `acp`; the event stream was **not exercised**.

## Secret references (verified in M7)
Both verified CLIs expand environment references in their MCP config: Claude Code `${NAME}` (in `env`, `args`, `headers`) and OpenCode `{env:NAME}` (in `environment`, the `command` array, `headers`). This is how Naurix keeps secret values out of the files it writes (ADR 0015). Details and method: `crates/core/tests/fixtures/agents/README.md`.

## Consequences for the next milestones
1. **M7 (MCP) — done (ADR 0015):** sync is implemented only for what is Verified: **Claude Code project `.mcp.json`** and **OpenCode flat shape**. Codex/Gemini/Antigravity adapters stay "documented only" until someone can run those CLIs and capture fixtures (or the maintainer confirms their formats). Every write is dry-run → approve → backup → reversible, and only touches entries Naurix created.
2. **M8 (Skills) — done (ADR 0016):** there is no account-free way to confirm that an agent *discovers* a skill, so the maintainer decided on 2026-11-19 that a reversible symlink into the documented folder is acceptable (level *Documented, accepted*). Note the overlap: OpenCode also reads `~/.claude/skills` and `~/.agents/skills`, and Codex and Antigravity share `.agents/skills`, so one central store can be symlinked once per distinct directory.
3. **Status:** stays *process state only*. Reliable "waiting/done" needs opt-in hooks (Claude/Codex) or an attached server (OpenCode), each a separate, explicitly approved feature.

## Open questions for the maintainer
1. Keep the added **Antigravity CLI** preset? (Recommended: yes.)
2. ~~For **skills** (M8), may a reversible symlink count as acceptable?~~ **Answered: yes** (2026-11-19, ADR 0016).
3. Do you have Codex / Gemini / Antigravity installed anywhere? A few `mcp add` runs would let us promote them from *documented* to *verified*.
