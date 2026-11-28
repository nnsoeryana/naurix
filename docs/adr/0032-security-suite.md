# ADR 0032 — Security Suite: reviewed reports, never stored findings
**Status:** Proposed (maintainer to accept; the four decisions were answered on 2026-11-28, see the end). **Partly implemented:** the Core report model, the SARIF reader and the comparison (§3–§5), and the exit codes and roles (§7). The bridge and the view (§8) and the offensive approval class (§6) are not built yet.

## Context
The maintainer asked for a **Security Suite** inside Naurix with three roles: a Red Team (offensive assessment), a Blue Team (defensive analysis and remediation) and a Purple Team (validating findings, correlating them, and catching security regressions). It must reuse what Naurix has, not add a runtime or platform of its own.

A read-only audit on 2026-11-28 (`docs/security/SECURITY_ARCHITECTURE_MAPPING.md`, `SECURITY_INTEGRATION_MATRIX.md`, `SECURITY_ARCHITECTURE_GAPS.md`) found:
- The reusable pieces are there. Custom agents (`agents/def.rs` `CustomAgent`) run any scanner as an executable plus an argument array through the approval gate. Workflows (ADR 0030) order steps. The skills manager (ADR 0016) installs a repository that holds several skills. The approval gate (ADR 0027) can gain action classes.
- The brief collides with the invariants in five places:
  - **Findings are content.** They carry file paths, messages and snippets that come from source code and tool output.
  - **Naurix never sends prompts or input to an agent** (ADR 0030 §4).
  - **Logic belongs in Rust, not TypeScript** (ADR 0002).
  - **Skill scripts are never run** (ADR 0016 §2).
  - **Nothing in Naurix knows what an offensive "target" is.**

This ADR is outside the nine stages of ADR 0022, so AGENTS.md "Scope control" requires it before any code. ADR 0030 and ADR 0031 were accepted on 2026-11-28 so this one does not build on proposals.

## Decision

### 1. What the suite is
The suite is a **user-driven viewer and comparer of security reports**, plus conventions for running scanners and agents through the features Naurix already has:
- **Red / Blue / Purple are roles, not runtimes.** A role is delivered as a **skill** that the user installs from a pack (a Git repository or folder of skills, ADR 0016) and links into an agent. The user then launches that agent and types the instructions. Naurix sends no prompt and no input.
- **Scanners are custom agents** (ADR 0011), run alone or as workflow steps (ADR 0030). Every launch passes the approval gate with a fingerprint over the program, the arguments and the environment.
- **Reports are SARIF 2.1.0 files** that a scanner or agent writes into the workspace. Naurix reads one **only when the user picks it**.

### 2. Findings are never stored (maintainer decision 1)
Naurix does not persist findings, report content, file paths taken from reports, or messages. **No migration and no table is added.** A report is read when the user opens it, shown, and dropped when the view closes. A Purple regression check compares **two reports that the user opens together**. The metadata-first invariant is unchanged and gains no exception. A consequence the user accepts: there is no finding history, no saved triage state, and nothing survives a reload or restart. The reports themselves stay in the workspace, owned by the user.

### 3. The report model (Core, `crates/core/src/security`)
A `Finding` holds:
- `tool` and `rule_id`
- `severity` (critical, high, medium, low or info)
- `level` (SARIF `error`, `warning`, `note` or `none`)
- `location`: a path **relative to the workspace root**, or none, plus the start line
- `message`
- `fingerprint`

There is no snippet, no region text, no code-flow data and no raw JSON. The model, the reader and the comparison are pure: they write nothing, start nothing and do not log.

**Severity:**
- Taken from `properties.security-severity` (a number from 0 to 10) when the result or its rule has it: 9 or more is critical, 7 or more is high, 4 or more is medium, more than 0 is low, and 0 is info.
- Otherwise taken from `level`: `error` is high, `warning` is medium, `note` is low, `none` is info.
- A missing level defaults to SARIF's `warning`.
- `security-severity` is a GitHub code-scanning convention, documented but not verified against real scanners here. It is labelled as such in the view.

### 4. Reading a report is reading untrusted input
- **Size:** the file must be 20 MiB or less, checked before reading and enforced while reading. At most 5,000 results are kept; the rest are counted, not kept.
- **Version:** only SARIF `version` 2.1.0, or a missing version with a `runs` array. Anything else is refused with a clear message. `serde_json` already limits nesting depth, and no new dependency is added.
- **Text:**
  - Control characters other than newline and tab are removed.
  - Limits: `rule_id` 200 characters, `tool` 100, path 1,024, message 500 (cut with an ellipsis).
- **Paths:**
  - Only the artifact URI is used. A `file://` URI or an absolute path is kept only if it is inside the workspace root, and then made relative.
  - A relative path is kept only if it contains no `..`, no root and no prefix component.
  - Checking is **lexical only**: the file system is never touched, so symlinks are not followed.
  - A path outside the workspace is dropped and the result is counted as `outside_workspace`.
- **Secrets:** each message goes through the memory secret scanner (`memory::secrets::scan`, ADR 0031 §4). If it matches, the message is replaced by "Message hidden: it looks like <kind>". Secret scanners often echo the secret they found, and the view must not repeat it.
- **Errors** name the problem (too large, wrong version, not JSON). They never quote report content.

### 5. Fingerprint and comparison (Purple regression)
- **Fingerprint:** SHA-256, length-prefixed (the same scheme as `approval::fingerprint`), over:
  - the tool, the rule id and the relative path;
  - then the result's `partialFingerprints`, sorted, if there are any;
  - otherwise the message, plus the occurrence number of that same key in the report.

  The line number is left out, so code moving within a file does not turn a finding into "new".
- **Comparison** of a baseline and a current report gives three lists: **new**, **still present** and **no longer reported**. Matching is by fingerprint, as a multiset. The view says plainly that "no longer reported" means only that the scanner stopped reporting it. It does not mean the issue was fixed or verified fixed.

### 6. Offensive targets (maintainer decision 2; not built in this ADR)
Red Team actions that reach beyond static files will need a new approval class, for example `run_offensive_check`, with **once** or **run** scope only. Its fingerprint will cover the tool and the exact target list. The **default target policy is the workspace folder and loopback** (`localhost`, `127.0.0.1`, `::1`). Any other target must be typed by the user and appear in the review. Until that class exists, Naurix starts no network-facing security action other than a custom agent the user reviewed, which is how things work today.

### 7. Scanner exit codes, and roles
Many scanners exit with a non-zero code when they find issues, and ADR 0030 §2 counts that as a failed step that blocks its dependents. A custom agent will gain an optional `success_exit_codes` list (default `[0]`), included in its launch fingerprint. Agent definitions that never set it keep exactly their earlier fingerprint, so existing approvals still match. A custom agent may also carry a **role** (`red`, `blue` or `purple`). It is a label for grouping and display only; it is not in the fingerprint because it does not change what runs. Both fields are optional JSON fields on the stored custom agent, so no migration is needed and agents saved earlier load unchanged.

### 8. Bridge and view (not built in this ADR)
- **Bridge:** `src-tauri/src/security.rs` will open the native file dialog itself (the ADR 0031 §7 pattern), so the webview never names a path. It reads the file through Core and returns the findings to the view.
- **View:** `src/features/security` will show the findings with severity filters, and a "compare with…" action.
- **Optional:** none of this is needed to use terminals or agents.

### 9. Not in scope
- Naurix calling MCP `tools/call`, which needs its own ADR.
- Automated prompts or headless agents (a capability must be verified per agent, ADR 0014).
- A pack manifest that carries agents, workflows or MCP servers.
- Passing data between workflow steps.
- Applying patches or committing fixes. Agents edit code under their own permissions, ideally in a worktree (ADR 0028).
- Storing findings, scheduled scans, and any background scanning.

## Consequences
- **Core** gains `security` (`finding`, `sarif`, `compare`). There is no migration, no new dependency and no Tauri dependency.
- **PRD** gains SEC-001 to SEC-006.
- **Tests:**
  - bounds (size, result count, text lengths);
  - version refusal;
  - hostile paths (`..`, absolute outside the root, `file://` outside the root, Windows-style prefixes);
  - control characters;
  - a secret in a message is hidden and never returned;
  - no snippet text is ever returned (a planted marker);
  - the severity mapping;
  - fingerprints are stable across a line shift and separate duplicates;
  - comparison of new, still present and no longer reported, including duplicates and empty reports.
- **Risks:**
  - Users may read "no longer reported" as "fixed". Mitigation: the wording in §5, and the model has no "fixed" state.
  - A hostile report may try to make the view show misleading paths. Mitigation: paths are lexical, relative and confined, or dropped.

## Decided by the maintainer (2026-11-28)
1. **Findings are never stored.** Reports are read when the user opens them; regression compares two reports opened together.
2. **The default target policy for later offensive actions is the workspace folder plus loopback.** Other targets must be typed by the user and appear in the review.
3. **ADR 0030 and ADR 0031 are accepted** before this ADR builds on them.
4. **First round:** this ADR, the PRD rows, and the Core model, reader and comparison. The bridge, the view and the agent changes come after review.

## Implementation log (2026-11-28)
Implemented in `crates/core/src/security` (`mod.rs`, `finding.rs`, `sarif.rs`, `compare.rs`), with tests in `crates/core/tests/security.rs`. No new dependency.

**P5 (2026-11-28):** custom agents have `role` and `success_exit_codes` (`agents/def.rs`: `Role`, `normalize_success_codes`, `launch_action`). The workflow engine reads the codes through a new default method, `Launcher::success_exit_codes`, when a step launches. The launcher editor offers both fields. Tests cover:
- validation and normalisation of the codes;
- the unchanged fingerprint for the default `[0]`, pinned against the earlier part list;
- a role that does not change the fingerprint;
- old stored agents loading without the fields;
- a step exiting 1 with `[0, 1]` succeeding, while another agent's exit 3 still fails.

**Not verified:**
1. No real scanner's SARIF was used. The fixtures are hand-written to the SARIF 2.1.0 shape, not captured from a tool with a recorded version (ADR 0014 evidence rule).
2. The `security-severity` convention was not checked against real scanner output.
3. There is no bridge or view yet, so nothing is reachable from the app.
4. Windows-style paths are handled lexically and tested on macOS only. Linux and Windows are left to CI.
