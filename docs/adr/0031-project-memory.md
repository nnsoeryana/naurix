# ADR 0031 — Opt-in project memory (orchestration stage 9)
**Status:** Proposed (maintainer to accept; the three decisions were answered on 2026-11-23, see the end). **Design only; no code.** Implements stage 9 of ADR 0022 (§3) and MEM-001 to MEM-007. MEM-008 (delivery to agents) is implemented in its smallest user-driven form (§6); writing to agents' own memory files is deliberately not part of this stage.

## Context
ADR 0022 §3 decided the shape of memory: SQLite with FTS5, explicit and opt-in per workspace, never automatic, user-controlled, exportable, never committed to Git, never delivered silently. This ADR fixes the details.

Facts checked on 2026-11-23: the bundled SQLite in the `rusqlite` build already in use is 3.54.1 and **FTS5 is compiled in** (a probe created an FTS5 table with the `unicode61` tokenizer and matched a row), so no dependency is added. The webview is not trusted to name file paths (ADR 0027/0028 pattern), and the dialog plugin is already used for folder picking.

## Decision

### 1. What a note is
A note has a **title** (1–100 characters) and a **body** (up to 8,000 characters), both written by the user in Naurix. Control characters other than newline and tab are removed. At most **200 notes per workspace**. There are no tags, no attachments and no links to files or sessions: a note is text the user typed, nothing is captured from anywhere.

### 2. Storage (migration 7)
- `workspaces.memory_enabled INTEGER NOT NULL DEFAULT 0` (opt-in per workspace; every existing workspace stays off).
- `memory_notes(id TEXT PRIMARY KEY, workspace_id → workspaces ON DELETE CASCADE, title, body, created_at, updated_at)`.
- `memory_fts`: an FTS5 table over `title` and `body` with `content='memory_notes'`, the `unicode61` tokenizer with `remove_diacritics 2`, and insert/update/delete triggers that keep it in step. Deleting a note, deleting all of a workspace's notes and **removing the workspace** (a cascade) all remove the index rows; each is tested, because "deleted" must also mean "no longer searchable".
- The database stays in the app data folder. Nothing is written to a workspace folder or a repository.

### 3. Opt-in and what "off" means (MEM-001)
Memory is **off** for every workspace until the user turns it on in the Memory view. While off, the view offers only the switch; notes cannot be added, searched or inserted. Turning it off **keeps** existing notes but hides them, and the view says how many are kept, with **Delete all notes** next to the switch, so nothing lingers invisibly without a way to remove it. Removing the workspace deletes its notes.

### 4. Secrets are refused, without an override (maintainer decision 2; MEM-003)
Before a note is saved, a scanner looks for likely credentials: private-key headers (`-----BEGIN … PRIVATE KEY-----`), AWS access key ids (`AKIA…`), GitHub tokens (`ghp_…`, `github_pat_…`), `sk-…` style API keys, Slack tokens (`xox…-`), Google API keys (`AIza…`), JWTs, and `name = value` assignments whose name suggests a secret (`password`, `secret`, `token`, `api_key`, `private_key`, `auth`) and whose value is at least 16 characters long and looks random. A match **rejects the save** and names the **line and the kind** ("line 3 looks like an AWS access key") but **never repeats the value**. There is no "save anyway". False refusals are possible; the user edits the text. The same scan runs when a note is edited. The scanner is a safety net, not a guarantee, and the UI says so. Secrets belong in the OS credential store (ADR 0013).

### 5. Search and the user's control (MEM-004, MEM-005)
Search runs through FTS5. The user's query is **never passed to FTS5 as written**: it is split into words, each quoted as a prefix term (`"word"*`), so FTS5 operators and syntax errors cannot occur; an empty query lists all notes, newest first. Results are ranked by relevance and capped at 50. Notes can be viewed, edited and deleted one by one, and all of a workspace's notes can be deleted at once after a confirmation.

### 6. Delivery to agents: a user-driven insert, nothing written (maintainer decision 1; MEM-008)
Each note has **Insert into agent terminal**. It types the note's text into the **active pane when that pane is an agent session**, through the terminal's paste (so bracketed paste is honoured), **without pressing Enter**. The user sees the text in the agent's input, can edit or discard it, and sends it with their own Enter. Properties:
- No file is written, no agent configuration changes, nothing needs rolling back, and nothing is sent without a click on a specific note.
- It is **refused for plain shell panes**: pasted newlines could run commands in a shell that lacks bracketed paste.
- The text goes through the terminal path only; Naurix does not store, log or inspect what the agent does with it.
- Writing memory into an agent's own memory files (CLAUDE.md, AGENTS.md and similar, with dry-run, backup and rollback) is **not** in this stage. It needs a verified target per agent and its own ADR.

### 7. Export (MEM-006, MEM-007)
- **Markdown** (a heading per note with its dates, then the body) and **JSON** (`workspace`, `exportedAt`, and per note `title`, `body`, `createdAt`, `updatedAt`; no ids and no paths).
- The **backend opens the save dialog itself**, so the destination comes from the OS dialog and never from the webview. If the chosen location is **inside a Git repository** (a `.git` entry in the folder or any parent, checked without running Git), the backend asks for a second, explicit confirmation naming the risk before writing. Naurix never runs `git add` or `git commit`, and never writes memory or an export anywhere the user did not choose.
- Export is a user action that writes only the file the user named, so it does not go through the approval gate.

### 8. Who can create notes (maintainer decision 3)
Only the user, in the Naurix UI. There is **no path for an agent to write a note**: no MCP tool, no hook, no output capture. Agent-proposed notes would need a channel from the agent and their own verification and ADR.

### 9. Privacy rules for the code
Note text is never logged, never put in an error message, and never included in an event. Errors name the field and the limit, not the content. No note content reaches the approval gate, workflows or status signals.

## Consequences
- Core gains `memory` (store, scanner, search, export, repository check) and migration 7. Tauri gains `memory_*` commands (including the export that opens its own dialog). The UI gains a Memory view and the insert action.
- Tests required: FTS5 is available; opt-in default off and per workspace; limits and cleaning; every scanner pattern and a set of ordinary text that must **not** be refused (code, hashes shorter than the threshold, prose about passwords); a refusal never contains the secret; search ranking, prefix matching and hostile queries (`"`, `*`, `AND`, `NEAR(`, `)`); deleting a note, deleting all, and removing the workspace each leave no index rows; disabling keeps notes hidden and re-enabling restores them; export Markdown and JSON content; the repository check; the insert is refused for a shell pane and never sends Enter.
- Risk: users paste secrets into notes. Mitigation: the scanner, the refusal without an override, and the plain statement that it is a safety net. Risk: an inserted note carries prompt-injection text to an agent. Mitigation: the user wrote it and sees it before sending.

## Decided by the maintainer (2026-11-23)
1. **Delivery: a user-driven "insert into the agent's terminal" without Enter.** No agent files are written in this stage.
2. **Suspected secrets are refused with no override.**
3. **Only the user creates notes**, in the Naurix UI.
