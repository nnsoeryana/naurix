import type { TerminalState } from "../terminal/TerminalView";
import type { PaneKind } from "../../state/store";
import type { MotionLevel, NaurixMode, NaurixSummary } from "./types";

/**
 * Naurix only reflects what Naurix can prove (AGENTS.md: never claim waiting/completed without a reliable
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
  /** Panes mid-turn. Omitted = every live agent counts as busy. */
  busyPanes?: ReadonlySet<string>,
): NaurixSummary {
  let running = 0;
  let busy = 0;
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
      if (!busyPanes || busyPanes.has(pane)) busy++;
      if (waitingPanes.has(pane)) waiting++;
    }
  }
  const mode = waiting > 0 ? "waiting" : failed > 0 ? "error" : busy > 0 ? "working" : running > 0 ? "ready" : "idle";
  return { mode, running, busy, failed, waiting };
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
export function displayMode(summary: NaurixSummary, celebrating: boolean): NaurixMode {
  if (summary.mode === "waiting" || summary.mode === "error") return summary.mode;
  return celebrating ? "success" : summary.mode;
}

export function statusText(mode: NaurixMode, s: NaurixSummary): string {
  switch (mode) {
    case "idle":
      return "Ready when you are. Let's build something!";
    case "working":
      return s.busy === 1 ? "Waiting for your agent's response..." : `Waiting for ${s.busy} agents...`;
    case "ready":
      return "Agent went quiet. Your turn!";
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
