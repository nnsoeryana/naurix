/**
 * Per-pane "turn" activity for Naurix. Metadata only: pane ids and timers, never terminal content.
 * A turn starts when the user submits input (Enter) or the pane launches, and ends once the pane
 * has produced no output for QUIET_MS. It is a heuristic ("output is flowing"), not an agent signal.
 */
export const QUIET_MS = 3500;

const busy = new Set<string>();
const timers = new Map<string, number>();
const listeners = new Set<() => void>();
let snapshot: ReadonlySet<string> = new Set();

function emit() {
  snapshot = new Set(busy);
  listeners.forEach((l) => l());
}

function arm(pane: string) {
  window.clearTimeout(timers.get(pane));
  timers.set(pane, window.setTimeout(() => endTurn(pane), QUIET_MS));
}

/** The user submitted input (or the agent was launched): a response is now expected. */
export function startTurn(pane: string) {
  const was = busy.has(pane);
  busy.add(pane);
  arm(pane);
  if (!was) emit();
}

/** Output arrived. Extends a turn in progress; output outside a turn (typing echo) is ignored. */
export function noteOutput(pane: string) {
  if (busy.has(pane)) arm(pane);
}

export function endTurn(pane: string) {
  window.clearTimeout(timers.get(pane));
  timers.delete(pane);
  if (busy.delete(pane)) emit();
}

export const subscribeActivity = (l: () => void) => (listeners.add(l), () => void listeners.delete(l));
export const getBusyPanes = () => snapshot;
