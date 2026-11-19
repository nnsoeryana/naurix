import type { TerminalState } from "../terminal/TerminalView";
import type { PaneKind } from "../../state/store";
import type { MotionLevel, NauriMode, NauriSummary } from "./types";

/**
 * Nauri only reflects what Naurix can prove (AGENTS.md: never claim waiting/completed without a reliable
 * adapter signal). Today the only evidence is the PROCESS state, so:
 *  - starting/running  -> "running" (active motion, but we do not claim what the agent is doing)
 *  - failed, or exit with a non-zero code -> error
 *  - exit by signal (user stopped it) or restored state -> nothing
 *  - exit with code 0 observed live -> a one-off success burst (see `detectCompletions`)
 *  - waiting: wired through `waitingPanes`, which stays empty until an adapter reports it.
 */
export function summarize(
  states: Record<string, TerminalState | undefined>,
  kinds: Record<string, PaneKind | undefined>,
  waitingPanes: ReadonlySet<string> = new Set(),
): NauriSummary {
  let running = 0;
  let failed = 0;
  let waiting = 0;
  for (const [pane, kind] of Object.entries(kinds)) {
    if (kind?.type !== "agent") continue;
    const st = states[pane];
    if (st?.kind === "failed") failed++;
    else if (st?.kind === "exited") {
      if (st.signal === null && st.code !== null && st.code !== 0) failed++;
    } else {
      running++;
      if (waitingPanes.has(pane)) waiting++;
    }
  }
  const mode = waiting > 0 ? "waiting" : failed > 0 ? "error" : running > 0 ? "working" : "idle";
  return { mode, running, failed, waiting };
}

/** Agent panes that just went from running to a clean exit. Compares two snapshots, so a restored/never-seen-running pane never fires. */
export function detectCompletions(
  prev: Record<string, TerminalState | undefined>,
  next: Record<string, TerminalState | undefined>,
  kinds: Record<string, PaneKind | undefined>,
): string[] {
  const out: string[] = [];
  for (const [pane, st] of Object.entries(next)) {
    if (kinds[pane]?.type !== "agent") continue;
    if (prev[pane]?.kind === "running" && st?.kind === "exited" && st.signal === null && st.code === 0) out.push(pane);
  }
  return out;
}

/** What is shown: waiting and error always outrank the temporary success burst. */
export function displayMode(summary: NauriSummary, celebrating: boolean): NauriMode {
  if (summary.mode === "waiting" || summary.mode === "error") return summary.mode;
  return celebrating ? "success" : summary.mode;
}

export function statusText(mode: NauriMode, s: NauriSummary): string {
  switch (mode) {
    case "idle":
      return "Ready when you are. Let's build something!";
    case "working":
      return s.running === 1 ? "Your agent is running..." : `${s.running} agents running...`;
    case "waiting":
      return "An agent needs your attention.";
    case "success":
      return "An agent finished cleanly.";
    case "error":
      return s.failed === 1 ? "An agent ended with an error." : `${s.failed} agents ended with errors.`;
  }
}

export function motionLevel(o: { enabled?: boolean; animations: boolean; reduced: boolean; systemReduced: boolean; hidden: boolean }): MotionLevel {
  if (!o.animations || o.systemReduced) return "off";
  if (o.hidden) return "paused";
  return o.reduced ? "reduced" : "full";
}

export const SUCCESS_MS = 2400;
