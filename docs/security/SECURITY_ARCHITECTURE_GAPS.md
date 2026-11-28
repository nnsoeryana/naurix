# Naurix Security Suite — Architecture Gaps, Conflicts and Plan

**Status:** Investigation only (2026-11-28). Proposals here are inputs for a future ADR; nothing is decided and nothing is implemented.
**Companion documents:** [SECURITY_ARCHITECTURE_MAPPING.md](SECURITY_ARCHITECTURE_MAPPING.md), [SECURITY_INTEGRATION_MATRIX.md](SECURITY_INTEGRATION_MATRIX.md).

---

## Decisions 2026-11-28 (they supersede parts of §4 and §5 below)
The maintainer decided the open points; [ADR 0032](../adr/0032-security-suite.md) records them:
- **C1: findings are never stored.** There is no migration 8, no `security_findings` or `security_imports` table, no triage state (G3, G4) and no `FindingsChanged` event (G11). A report is read when the user opens it. Purple regression compares two reports that are opened together.
- **C6: the default target policy is the workspace folder plus loopback.** It applies when the offensive approval class (G8) is built.
- **ADR 0030 and ADR 0031 are accepted** (P0 done).
- **Built in this round:** P1 (ADR 0032 and PRD SEC-001 to SEC-006), plus P2 and P4 in Core (`crates/core/src/security`: finding model, bounded SARIF reader, comparison). P3 and P6 are dropped.
- **Next, after review:** P5 (custom agent `role` and `success_exit_codes`), P7 (Tauri bridge with a backend-owned file dialog), P8 (Security view).

## 1. Process gate (before any code)

1. **The Security Suite is not one of the ADR 0022 stages.** AGENTS.md "Scope control" allows orchestration work only in stages 1–9 and says anything else needs explicit request **and a new ADR**. The next ADR number is **0032**.
2. **Stages 8 and 9 are still "Proposed".** ADR 0030 (workflows) and ADR 0031 (memory) are implemented but their status line says "maintainer to accept". The suite builds on both; they should be accepted first so the suite does not stack on unaccepted decisions.
3. **PRD requirement IDs** must be added (new `SEC-xxx` rows in `docs/PRD.md` §5.9 or a new §5.10).

## 2. Architectural conflicts

Each conflict names the invariant (AGENTS.md / ADR) and what the brief assumes.

| ID | Conflict | Invariant / source | Resolution options |
|---|---|---|---|
| **C1** | **Findings are content.** Rule ids are fine, but file paths, line numbers, code snippets and tool messages derive from source and tool output. | AGENTS.md "Metadata-first persistence"; ADR 0022 §3.3 (only user-authored memory is an exception) | (a) New ADR granting a **narrow, opt-in exception**: normalized fields only (tool, rule id, severity, relative path, line, a ≤500-char message, a stable fingerprint), no snippets, no raw output, per-workspace opt-in, delete-all, removed with the workspace, secret scan on save. (b) Keep findings out of the DB entirely and only show them from a user-chosen file per view (read, never stored). **Recommend (a) with SARIF-only import**, user-triggered. |
| **C2** | **No automated prompts or input.** Red/Blue/Purple "agents" that act on their own need prompts sent by Naurix. | ADR 0030 §4 (steps store no prompt, send no input); ADR 0027 §2 (`SendInput` deferred); ADR 0022 §5 ("an autonomous agent that acts without approval" out of scope) | Roles are delivered as **skills** linked into the agent (D1/D2) plus user-typed instructions. Automated prompting stays out of the first slice. |
| **C3** | **"TypeScript conventions"**. | AGENTS.md engineering rules; ADR 0002 | All suite logic (finding model, import, storage, approval) in Rust `crates/core/src/security`; TypeScript only for the view in `src/features/security`. |
| **C4** | **Naurix calling MCP tools.** The brief's "Hybrid CLI + MCP through adapter abstraction" implies Naurix is an MCP client at runtime. | `mcp/probe.rs` only handshakes; ADR 0015; "Never run an unapproved MCP server" | First slice: MCP tools are used **by agents** (A6). Naurix-side `tools/call` needs its own ADR, action class, output bounds and an output-is-untrusted parser. |
| **C5** | **Skill pack executing scanners.** | AGENTS.md "Never execute skill scripts"; ADR 0016 §2 | A pack ships skills (instructions). Executables run only as **custom agents** the user defines and approves (fingerprint over path + args). |
| **C6** | **Offensive actions against targets.** Red Team assessment may touch network targets; nothing in Naurix knows what a "target" is. | AGENTS.md "User control"; ADR 0022 §4 | New approval class with an explicit **target list in the fingerprint**, once/run scope only, and a default target policy of **the workspace folder and loopback only**. Anything else must be typed by the user and shown in the review. |
| **C7** | **Exit-code semantics.** Scanners commonly exit non-zero when findings exist, which the engine treats as `failed` and blocks dependents. | ADR 0030 §2 | Per-custom-agent `success_exit_codes` (default `[0]`), included in the launch fingerprint. |
| **C8** | **Memory secret scanner vs security notes.** Notes that quote token shapes (e.g. a finding about a leaked `AKIA…` key) are refused with no override. | ADR 0031 §4 (maintainer decision: no override) | Accept as is; tell users to describe, not paste. Do not weaken the scanner. |
| **C9** | **"Agent completed" claims.** A Purple "validation passed" cannot be inferred from output or status signals. | AGENTS.md; ADR 0030 §2 | Validation results come only from exit codes, SARIF import and the user's decision. |
| **C10** | **Git mutations for fixes.** | AGENTS.md "do not mutate Git state"; ADR 0028 | Blue team edits happen inside the agent (its own permissions), ideally in a Naurix worktree created with a once approval. Naurix does not commit. |

## 3. Missing capabilities

| ID | Capability | Needed for | Smallest form |
|---|---|---|---|
| **G1** | Finding model + normalizer | Blue/Purple triage, regression | `crates/core/src/security/finding.rs`: struct, validation, stable fingerprint (tool + rule + relative path + normalized location), length limits |
| **G2** | SARIF 2.1.0 importer (untrusted input) | Any scanner | Parse with `serde_json` (no new dependency), hard caps (file ≤ 20 MB, ≤ 5,000 results, string lengths), paths normalized and confined to the workspace root, unknown fields ignored |
| **G3** | Findings storage | Persistence across sessions | Migration 8 in the existing DB: `security_findings` (+ `security_imports` header row linking to `runs.id` nullable); `ON DELETE CASCADE` from `workspaces` |
| **G4** | Triage state | Blue team | Column `state` (`open`, `accepted_risk`, `false_positive`, `fixed`) + `updated_at`; user-only changes |
| **G5** | Tool definitions (security CLI catalog) | Hybrid CLI execution | Reuse `CustomAgent`; add optional `role` (`red`/`blue`/`purple`/none) and `success_exit_codes`. **Not** a new adapter trait |
| **G6** | Secrets for custom agents | Token-based scanners | Keychain-backed env for custom agents, mirroring `McpStore::launch_environment` (`mcp/store.rs:215`); values never in SQLite or the review text, names in the fingerprint |
| **G7** | Purple correlation / regression | Purple team | Pure function over two imports: new / still present / fixed by fingerprint. No AI, no heuristics on output |
| **G8** | Offensive target authorization | Red team | `ActionClass::RunOffensiveCheck` (once/run), fingerprint over tool + target list |
| **G9** | Workflow data passing | Automatic Red → Blue handoff | **Deferred.** Use user-triggered SARIF import after a step instead |
| **G10** | Pack manifest (skills + agent definitions + workflows) | "Standalone installable" beyond skills | **Deferred.** First slice: pack = skills only (already supported) + documented custom agent definitions the user creates |
| **G11** | `FindingsChanged` event | UI refresh | Add variant to `events.rs:18` with ids only |
| **G12** | Security view | UI | `src/features/security/` (list, filters, triage, import button, regression diff) |

Explicitly **not** needed (avoid speculative abstractions, AGENTS.md "Scope control"): a new database, a new runtime, a new adapter trait, a plugin system, a background scanner, scheduling.

## 4. Recommended smallest changes (first vertical slice)

Goal: a developer can run a scanner from Naurix, import its SARIF report, triage findings, and compare a later run (regression) — with every invariant intact.

1. **ADR 0032 — Security Suite scope and findings storage.** Decides C1 (narrow exception), C3, C5, C6 (default target policy), C7, roles as metadata, SARIF as the only import format, and what stays deferred (G9, G10, A7, A9, A10).
2. **`CustomAgent` extension** (`crates/core/src/agents/def.rs`): optional `role`, `success_exit_codes` (validated, in the launch fingerprint `launch_action` `:170`); workflow engine maps exit codes via the run step's snapshot. Migration adds columns or JSON fields consistent with how custom agents are stored today (`agents/store.rs:42-88` — verify storage form before choosing).
3. **`crates/core/src/security`** (new Core module, no Tauri): `finding.rs` (model, fingerprint), `sarif.rs` (bounded importer), `store.rs` (`SecurityStore<'a>`), `regression.rs` (diff of two imports).
4. **Migration 8**: `security_imports`, `security_findings`; per-workspace opt-in column `workspaces.security_enabled` (same pattern as `memory_enabled`, migration 7).
5. **Import is user-driven**: the backend opens the file dialog itself (ADR 0031 §7 pattern), so the webview never names a path; the file is read once, parsed, discarded.
6. **Tauri bridge** `src-tauri/src/security.rs`: `security_status`, `security_set_enabled`, `security_import`, `security_list`, `security_triage`, `security_regression`, `security_delete_all`. Validation only; delegation to Core.
7. **Event** `Event::FindingsChanged { workspace_id, import_id }`.
8. **UI** `src/features/security/` with `api.ts`, `SecurityView.tsx`, `model.ts`, tests.
9. **Skill pack** (outside this repository): a Git repo with `red-team/SKILL.md`, `blue-team/SKILL.md`, `purple-team/SKILL.md` instructing agents to write SARIF to a known relative path. Installed through the existing Skills manager. No scripts required; any scripts are flagged and never run by Naurix.

Second slice (separate ADRs, only if wanted): G6 keychain env for custom agents; G8 offensive target approval; then A9 headless capability per agent (needs a verified fixture); then G9/G10.

## 5. Dependency-aware implementation plan

```
P0  Maintainer: accept ADR 0030, ADR 0031                     (no code)
P1  ADR 0032 + PRD SEC-xxx rows                               (docs)        ← needs P0
P2  Finding model + SARIF importer + unit tests               (Core)        ← needs P1
P3  Migration 8 + SecurityStore + integration tests            (Core)        ← needs P2
P4  Regression diff (pure) + tests                            (Core)        ← needs P2 (data), P3 (read)
P5  CustomAgent role + success_exit_codes + engine mapping    (Core)        ← needs P1; independent of P2–P4
P6  Event FindingsChanged                                     (Core)        ← needs P3
P7  Tauri commands (backend-owned file dialog)                (src-tauri)   ← needs P3, P4, P6
P8  Security view + Vitest                                    (src)         ← needs P7
P9  Skill pack repo (external) + manual run with a real scanner and a real agent, recorded in ADR log
                                                              (external)    ← needs P5, P8
P10 Optional: G6 keychain env (ADR 0033), G8 offensive target class (ADR 0034)   ← after P9
```

P2–P4 and P5 can proceed in parallel after P1. Each phase leaves the app runnable with existing tests green (AGENTS.md).

### Tests required per phase
- **P2:** SARIF with hostile content (huge strings, absolute paths, `..` traversal, paths outside the workspace, deeply nested JSON, wrong types) is bounded or rejected; fingerprint stable across runs and line-shift tolerant as designed; no snippet field is kept.
- **P3:** opt-in default off; per-workspace isolation; workspace removal cascades; delete-all; planted-marker test proving no snippet/raw text reaches the DB files; secret scanner (`memory/secrets.rs` reuse) applied to messages.
- **P4:** new / unchanged / fixed classification; empty and identical imports.
- **P5:** exit code in the accepted list → `succeeded`; other codes → `failed`; changing the list changes the fingerprint and invalidates an earlier approval.
- **P7/P8:** webview cannot name a path; triage only accepts known states; view not required for terminal use (smoke harness).
- **P9:** a real scanner and a real agent recorded with versions (ADR 0014 evidence rule); listed as "Not verified" otherwise.

## 6. Open questions (UNKNOWN / NEEDS INVESTIGATION)

1. Which scanners are targeted first, and do they emit SARIF 2.1.0 with stable `ruleId` and `partialFingerprints`? (Needs real fixtures with versions.)
2. Does any supported agent discover a symlinked skill? (ADR 0016: not verified.)
3. Is any agent's headless mode verifiable for unattended security review? (Feature does not exist; per-agent fixture required.)
4. Should findings be per workspace only, or also per worktree (ADR 0028)?
5. Maintainer decision on C1 (findings storage exception) and C6 (default target policy) — these decide whether the suite is feasible within the invariants.
6. Retention: cap on imports/findings per workspace (memory uses 200 notes; findings likely need a higher, still bounded cap).
