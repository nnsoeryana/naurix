# Agent config fixtures

Files written by the **real CLIs** in a throwaway `HOME` and project folder (nothing of the
maintainer's own configuration was read or modified). Tests use them to check that Naurix's
claims about each agent's format match what the agent itself produces. Re-capture when an agent
changes; never edit them by hand.

| File | Produced by | Command (run in an empty temp dir with a temp `HOME`) |
|---|---|---|
| `claude-2.1.322-project.mcp.json` | Claude Code 2.1.322 | `claude mcp add --scope project --transport http web https://example.com/mcp --header "X-Demo: 1"` then `claude mcp add --scope project demo -e DEMO_KEY=abc -- echo hello` |
| `opencode-1.18.10-global.opencode.jsonc` | OpenCode 1.18.10 | `opencode mcp add demo --env DEMO_KEY=abc -- echo hello` (it wrote the **global** file, with the flat `mcp.<name>` shape, not the `mcp.servers` shape shown in current docs) |

Captured 2026-11-19 on macOS 14.6 (x86_64). Gotcha: Claude's `--env` is variadic, so
`--env KEY=v name -- cmd` swallows the name; put `-e KEY=v` after the name.

## Secret references (verified 2026-11-19, M7)

Both CLIs expand environment references inside their MCP config, which is what lets Naurix keep
secret values out of the files it writes. Checked with a stand-in server that logs what it received
(and, for headers, a local HTTP server that logs the request headers), run in a throwaway `HOME`:

| Agent | Syntax | Verified places |
|---|---|---|
| Claude Code 2.1.322 | `${NAME}` | `env` values, `args` elements, HTTP `headers` |
| OpenCode 1.18.10 | `{env:NAME}` | `environment` values, `command` array elements, `headers` |

If the variable is not set, Claude Code passes the literal `${NAME}` text to the server and prints a
config warning (so a missing secret fails visibly without exposing anything).
