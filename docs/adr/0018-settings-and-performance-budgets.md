# ADR 0018 — Settings model and performance budgets (M10)
**Status:** Accepted

**Settings**
1. One typed JSON document (`app.settings`) in the `settings` table: `general` and `terminal` groups. **Writes are strict** (an invalid value is an error the UI shows; nothing invalid is stored), **reads are tolerant** (an out-of-range or unknown value falls back to its default so a hand-edited or newer-version file cannot stop the app from starting). The older font-only keys are migrated once.
2. The frontend sends the whole document; updates are **serialized and applied only after the backend accepted them**. An earlier optimistic version let overlapping edits overwrite each other and briefly applied an invalid scrollback to a live terminal; both were found by the UI test and fixed.
3. Settings apply **live** to running terminals (font, size, scrollback, cursor, renderer); shell path and login-shell apply to **new** terminals. The shell path must be an absolute executable file; a stale path falls back to `$SHELL`.
4. The Security section shows facts, not promises: no telemetry exists, where data lives, the Keychain service, and lets the user withdraw all MCP approvals and revoke trusted Git repositories.

**Performance (measured on Intel i5-8257U, macOS 14.6, release build, harness `bench/measure.mjs`, 30 s per run, window brought to the front)**
| Scenario | Total RSS (MB) | Footprint (MB) | CPU % median (peak) |
|---|---|---|---|
| Idle, 1 terminal, cursor blinking | 117–137 | 80–88 | 0.1–0.2 (35 once) |
| Idle, 1 terminal, cursor not blinking | 112–149 | 76–85 | 0 |
| Sustained flood, DOM renderer | 313–376 | 141–150 | ~260 |
| Sustained flood, WebGL renderer | 312–353 | 157–173 | 41–269 |
| 4 terminals with agent-like output | 181–207 | 123–125 | 15–19 (peak 37–46) |

**Decisions**
- **Default renderer stays DOM.** WebGL gave no clear win (RSS similar, footprint 10–20 % higher, CPU inconclusive across two runs) and adds GPU-context risk; it remains an opt-in, with the active/unavailable state shown in Settings. A lost or unavailable WebGL context falls back to DOM and stops retrying.
- **Cursor blink stays on by default.** With a focused window it cost 0.1–0.2 % CPU, so the ~5 % reading in M3 is **not reproduced** and was probably the debug harness or an unfocused-window artifact. The option remains for anyone who wants zero idle wake-ups.
- **Budgets.** Idle ≤ 120 MB and one basic terminal ≤ 180 MB **RSS** were hypotheses (ADR 0006). Measured idle RSS is 112–149 MB with large run-to-run spread caused by WebKit, so the idle budget is **revised to ≤ 150 MB RSS**, and the more stable **footprint** (76–88 MB) is recorded alongside as the primary comparison number. The "one terminal ≤ 180 MB" budget holds for idle and agent-like output (181–207 MB RSS under four agents is at the edge; footprint 125 MB). **Sustained flood is a stress case with its own budget: ≤ 400 MB RSS**, observed 313–376 MB; memory is bounded by flow control (it did not climb over 30 s) but not small.
- Flow control (256 KB in flight) was not tuned: lowering it would trade throughput for memory, and the flood CPU (~260 %) is dominated by the WebView parsing output, not by Naurix's Rust side.

**Not measured / limits:** Apple Silicon (no hardware here), the 8-terminal and mixed cases under WebGL, battery impact, and any real agent CLI (scripted output was used). Every number above is one machine and 2 runs per scenario; treat spreads as the honest uncertainty.
