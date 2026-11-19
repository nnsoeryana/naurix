# Naurix — Implementation Plan

**Progress:** M0–M10 done, plus the Naurix rename and brand UI (ADR 0013). Next: M11a (Apple Silicon validation on real hardware). M11b (signing, notarization) is deferred until an Apple Developer Program account exists.

Each milestone leaves the app runnable and is independently testable. Requirement IDs refer to PRD.md.

| M | Scope | Approach | Depends | Acceptance | Tests |
|---|---|---|---|---|---|
| **M0** | Repo hygiene and docs | git, LICENSE, CONTRIBUTING, SECURITY, ADRs, toolchain pins | — | Docs consistent | Link/consistency review |
| **M1** | Tauri 2 + React/TS scaffold, pnpm, CI on both archs, bench harness, minimal CSP/capabilities | `create-tauri-app`, then strip; harness samples app+helper RSS | M0 | `pnpm tauri dev` launches; idle RSS baseline recorded on x86_64 and arm64 | CI build; harness smoke |
| **M2** | PTY + supervisor (TERM-001,004,005) | `portable-pty` (verify in spike A), own process group, reader thread, `killpg` on close | M1 | Spawn/IO/resize/exit/kill; no orphans (Spike D) | Rust integration tests |
| **M3** | xterm.js view, batching, flow control, 5k scrollback (TERM-002,003,009) | Channel of bytes, ack backpressure, refs not state | M2 | `yes` flood stays responsive and memory bounded; 1-terminal footprint measured | Stress test; vitest attach/detach |
| **M4** | SQLite + workspaces, tabs, splits, palette, restart semantics (WS-*, TERM-006,007,008, DATA-*, UX-001,002) | migrations; sessions marked `interrupted` on startup | M3 | Workspaces persist; missing dir handled; no transcript stored | Repo tests; e2e restart |
| **M5** | Agent launch, presets, custom agent, exe detection, honest status (AG-001–008) | generic PTY adapter; status = process state only | M4 | Mock-agent launch/stop/exit shown correctly | Mock CLI in CI |
| **M6** | Adapter layer; spike C verifies each agent's real CLI behavior and MCP/skill locations from current docs | `AgentAdapter` + capability matrix | M5 | Matrix doc committed; unverified = unsupported | Adapter unit tests |
| **M7** | MCP manager (MCP-*) | registry, Keychain, approval fingerprint, tester, sync plan/apply/rollback | M6 | Fixture configs with user entries are never overwritten; rollback restores bytes | Sync fixtures; keychain test on both archs |
| **M8** | Skills manager (SK-*) | staging → confirm → store → symlinks | M6 | Reversible install/uninstall; scripts never executed; existing file blocks | Fixture + symlink tests |
| **M9** | Git status (GIT-001,002,004,007; diff P1) | `git` CLI porcelain v2 | M4 | Non-repo, detached HEAD, conflicts, missing git handled | Parser fixtures |
| **M10** | Settings UI (7 sections), perf tuning | wire existing services | M5–M9 | All benchmark scenarios recorded; budgets confirmed or revised by ADR | Bench results; UI tests |
| **M11a** | Release validation (Intel + Apple Silicon) | manual checklists, benchmark runs on an arm64 Mac | M10 | Compatibility matrix filled from real runs on both archs (unsigned builds, recorded as such) | Checklist records; `bench/results/` arm64 JSON |
| **M11b** | Signing and notarization | Developer ID signing, `notarytool`, `stapler` | M11a | `spctl`/`stapler` output recorded; Keychain tested with the signed build. **Blocked:** needs an Apple Developer Program account (not available yet) | Checklist records |

Spike A (xterm.js+PTY) lands in M2–M3, Spike D in M2, Spike C in M6, Spike B (libghostty) after M10 as a decision record.
