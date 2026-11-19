# Naurix — Performance and Compatibility

**Status:** Draft. All numbers below are **provisional hypotheses** (ADR 0006) until measured.

## 1. Metric definition

"Application RSS" = main process + WebKit helper processes (WebContent, GPU, Networking) belonging to the app. Excludes shells and agents the user launches. Measured with `ps -o rss` / `footprint` sampled by `bench/` harness; both RSS and `footprint` (phys_footprint) are recorded because RSS over-counts shared pages on macOS.

## 2. Provisional targets

| Scenario | Target (provisional) |
|---|---|
| Idle, no terminal | ≤ 120 MB |
| One basic shell session | ≤ 180 MB |
| 4 / 8 terminals | No target yet — record growth per terminal, expect roughly bounded by scrollback |
| Scrollback default | 5,000 lines |

Targets are revised only via a new ADR backed by recorded data. If unachievable on WKWebView we say so.

## 3. Strategy
- One WebView; one xterm.js instance per *visible* pane; hidden tabs keep the PTY and detach/dispose the view (the buffer is not duplicated in Rust or React).
- Rust coalesces PTY output (~16 ms / ≤64 KB frames), sends raw bytes over a Tauri `Channel`; ack-based flow control with a bounded queue so a flood (`yes`) cannot grow memory.
- No polling: Git refresh on window focus and session exit/explicit refresh; no filesystem watcher in the MVP.
- Terminal data never enters React/Zustand state; SQLite holds metadata only.
- Keep dependencies minimal; track binary size per release.

## 4. Benchmark plan (`bench/`)
Scenarios: idle; 1, 4, 8 terminals; 4 concurrent mock agents; sustained output (`yes`, large `cat`); 30-minute mixed workload. Per scenario record: RSS and footprint (app + helpers), CPU %, input-to-echo latency, output throughput, frame drops where observable. Results stored as `bench/results/<date>-<arch>-<commit>.json`, with a short summary in this document. Run each scenario ≥3 times; report median and range.

## 5. Compatibility matrix

| Item | x86_64 | arm64 | Evidence required to claim "works" |
|---|---|---|---|
| Build from clean checkout | | | CI log |
| Launch, shell, resize, paste | | | Manual checklist run on real hardware |
| PTY integration tests | | | `cargo test` run on that arch |
| Process cleanup | | | Spike D report |
| Keychain access | | | Manual test with signed build |
| Each agent CLI launch | | | Spike C, per agent version |
| Benchmarks | | | Result files above |
| Signed + notarized build | | | `spctl`/`stapler` output |

Hardware: maintainer's Intel Mac (i5-8257U, macOS 14.6) and an Apple Silicon Mac. Cells stay empty until a recorded run exists. Minimum macOS version is decided in M1 from Tauri 2/WKWebView requirements and CI availability.

### 5.1 Linux and Windows (ADR 0021)

| Item | Linux x86_64 | Linux arm64 | Windows x86_64 | Windows arm64 | Evidence required to claim "works" |
|---|---|---|---|---|---|
| Build from clean checkout | | | | | CI log |
| Launch, shell, resize, paste | | | | | Manual checklist on real hardware (include Wayland and X11 on Linux) |
| PTY integration tests | | | | | `cargo test` on that OS (Windows: `pty_windows.rs` only) |
| Process cleanup | | | | | Spike D report (Windows: `taskkill /T` tree kill) |
| Secret store | | | | | Manual test: Secret Service with and without a keyring daemon; Credential Manager |
| Skill links | n/a | n/a | | | Manual test with and without Developer Mode |
| Each agent CLI launch | | | | | Spike C per agent and OS (config paths are verified on macOS only) |
| Benchmarks | | | | | `bench/results/<date>-<os>-<arch>-…` (the harness uses macOS `ps`/`footprint` and needs porting first) |

All cells are empty: the code exists but nothing has been run on these systems.

## 6. CI
GitHub runners `macos-13` (x86_64) and `macos-14` (arm64) run `cargo test`, `clippy`, frontend tests. `.github/workflows/ci.yml` also runs the same checks on `ubuntu-24.04` and `windows-latest` (not yet run). CI proves the build and automated tests per arch; it does not replace the manual checklist or benchmarks.

## 7. Recorded results

### M1 baseline — placeholder UI (React + Tailwind, no terminal), release build, `x86_64`
Hardware: Intel Core i5-8257U, macOS 14.6, 4 processes (main + 3 WebKit helpers). Harness: `bench/measure.mjs`, 30 s sampling after 5 s settle, median per run. Raw data in `bench/results/`.

| Run | Total RSS (MB) | Total footprint (MB) | CPU % |
|---|---|---|---|
| 1 | 81.1 | 52.4 | 0 |
| 2 | 95.2 | 60.3 | 0 |
| 3 | 93.3 | 50.6 | 0 |

Reading: idle RSS 81–95 MB is under the provisional 120 MB target, but this has no terminal, xterm.js or SQLite yet, so it is only a floor. Run-to-run spread is ~14 MB. Helper attribution uses a PID snapshot (see harness header) and is contaminated if other WebKit apps start during a run. **arm64: not yet measured.**

### M3 — one terminal (xterm.js DOM renderer, 5,000-line scrollback), release build, `x86_64`
Same hardware and harness; `SHELL=/bin/sh`, app + WebKit helpers only (the shell itself is excluded). Raw data: `bench/results/*1terminal*`.

| Run | Total RSS (MB) | Total footprint (MB) | CPU % |
|---|---|---|---|
| 1 | 112.6 | 73.6 | 4.4 |
| 2 | 110.5 | 64.5 | 5.4 |
| 3 | 109.8 | 67.8 | 5.3 |

Reading: ~110 MB RSS is below the provisional 180 MB target, about +20 MB over the M1 placeholder. **Open issue:** ~5 % CPU while idle with one terminal, most likely the blinking cursor on the DOM renderer; to be investigated in M10 (try `cursorBlink: false`, then the WebGL addon and compare memory). Not yet measured: sustained-output memory in the release build (functional flood test only: `seq 1 300000` rendered completely through the real app, debug build, main-process RSS ~59 MB afterwards), 4 and 8 terminals (needs tabs, M4), arm64.

### M4 — multiple terminals and sustained output, release build, `x86_64`
Same hardware/harness; `SHELL=/bin/sh`; terminals opened with the benchmark mode (ADR 0010 §8); one tab visible, the rest hidden but mounted; 30 s sampling after a 10 s settle; 3 runs each. Raw data: `bench/results/*m4-*`.

| Scenario | Total RSS (MB) | Total footprint (MB) | CPU % |
|---|---|---|---|
| 1 terminal | 90.4 – 118.7 | 66.4 – 73.8 | ~0 |
| 4 terminals | 123.4 – 124.3 | 71.1 – 78.1 | ~0 |
| 8 terminals | 131.8 – 132.4 | 82.0 – 82.6 | ~0 |
| 1 terminal, endless `seq` flood (30 s) | 233.3 – 272.4 | 144.5 – 166.7 | 10 – 255 (median 10–15, one run 255) |

Reading:
- **Per-terminal cost is small:** ≈ +2 to +3 MB RSS per extra idle terminal (1→8: ≈ +14–40 MB depending on the 1-terminal run; WebKit's baseline varies run to run by ~±15 MB).
- **Idle CPU is ~0 here**, unlike the M3 measurement (~5 %). The M3 runs likely had a visible, focused, blinking cursor; these runs had the window in the background. Idle CPU with a focused window is therefore still unmeasured.
- **Sustained flood costs memory and CPU:** ~235–270 MB RSS, above the provisional 180 MB one-terminal target (which was defined for a basic idle session). Samples fluctuate rather than climb steadily over the 30 s (e.g. 224 → 246 → 271 → 275 → 187 MB), but a 30-minute run has **not** been done, so boundedness over time is unproven. Main suspects: WebContent parsing/rendering with the DOM renderer. Candidates for M10: WebGL renderer, lower flood batch sizes.
- Not measured: arm64; four concurrent *agents* (M5); the 30-minute mixed workload; a focused-window idle run.

### M10 — renderer, cursor blink, flood and four agents, release build, `x86_64`
Window brought to the front (`--focus`), 30 s per run after a 10 s settle, 2 runs each; raw data `bench/results/*m10-*`. Full table, interpretation and the revised budgets are in [ADR 0018](adr/0018-settings-and-performance-budgets.md).

| Scenario | Total RSS (MB) | Footprint (MB) | CPU % |
|---|---|---|---|
| Idle, cursor blinking | 117–137 | 80–88 | 0.1–0.2 |
| Idle, cursor not blinking | 112–149 | 76–85 | 0 |
| Flood, DOM | 313–376 | 141–150 | ~260 |
| Flood, WebGL | 312–353 | 157–173 | 41–269 |
| 4 agent-like terminals | 181–207 | 123–125 | 15–19 |

Reading: the ~5 % idle CPU seen in M3 does not reproduce with a focused window; WebGL does not beat DOM here, so DOM stays the default; sustained flood is bounded (~313–376 MB RSS) but above the old 180 MB one-terminal budget, which now applies to idle/agent-like use only (flood has its own budget). Still unmeasured: arm64, real agent CLIs.

### M10 — 30-minute mixed load, 4 terminals, release build, `x86_64`
Each terminal loops `seq 1 50000` (a burst), then idles 6 s. One run (332 samples, `bench/results/2026-11-19-x64-m10-mixed-30min.json`); the machine was also used for other work during the run (tests, type checks), so CPU is indicative only.

| Point in run | Total RSS (MB) | Footprint (MB) | Web processes |
|---|---|---|---|
| start | 393 | 298 | 4 |
| 25 % | 346 | 234 | 4 |
| 50 % | 423 | 264 | 4 |
| 75 % | 396 | 327 | 7 |
| end | 477 | 451 | 7 |
| **median** | **389** | **274** | CPU median 23.9 % |

**Finding (open):** footprint trended upward (298 → 451 MB) and WebKit helper processes went from 4 to 7 during the run. Scrollback is capped (5,000 lines per terminal), so unbounded line growth is not the explanation; candidates are WebKit memory not being returned to the OS (not necessarily a leak), the DOM renderer's retained nodes, or helper-process churn. It is **not** a pass for a long-session memory budget. Next step (M11a): repeat with the DOM vs WebGL renderer separately, a longer run, and a heap/footprint breakdown before claiming either a leak or its absence.
