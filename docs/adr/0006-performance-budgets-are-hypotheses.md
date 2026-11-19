# ADR 0006 — Performance budgets are hypotheses until benchmarked
**Status:** Accepted
**Context:** The brief sets provisional targets (idle ≤120 MB, one terminal ≤180 MB RSS). The PRD forbids claiming unverified budgets. Tauri on macOS uses WKWebView, whose WebContent/GPU helper processes are separate from the app process.
**Decision:** "Application RSS" is defined as the sum of the main process + its WebKit helper processes, excluding user-launched child processes (shells/agents). Targets stay labeled *provisional* in docs and are changed only with recorded measurements. A benchmark harness is built in M1, before features. Results are published per architecture.
**Consequences:** If targets prove unachievable we revise them openly (new ADR) rather than hide the metric.
