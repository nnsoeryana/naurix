import type { TerminalState } from "../terminal/TerminalView";

/**
 * Honest agent status (PRD AG-005/AG-009, ADR 0029). Without a signal the only thing we know is the
 * PROCESS state. "Working", "waiting for approval", "waiting for input" and "turn finished" appear
 * only when the agent itself reported them through a verified adapter signal (opt-in), and are then
 * labelled with the time of the last report: a report, not a live check.
 */
export type AgentSignalKind = "working" | "awaiting_approval" | "idle" | "turn_done";
export type AgentSignal = { kind: AgentSignalKind; at: string };

export type AgentStatus = {
  label: "Starting" | "Running" | "Exited" | "Failed" | "Working" | "Waiting for approval" | "Waiting for input" | "Turn finished";
  detail: string;
};

const SIGNAL_LABEL: Record<AgentSignalKind, { label: AgentStatus["label"]; what: string }> = {
  working: { label: "Working", what: "started working" },
  awaiting_approval: { label: "Waiting for approval", what: "asked for approval" },
  idle: { label: "Waiting for input", what: "went idle" },
  turn_done: { label: "Turn finished", what: "finished its turn" },
};

export const isAgentSignalKind = (k: string): k is AgentSignalKind => k in SIGNAL_LABEL;

function clock(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleTimeString();
}

export function agentStatus(s: TerminalState | undefined, signal?: AgentSignal): AgentStatus {
  switch (s?.kind) {
    case undefined:
    case "starting":
      return { label: "Starting", detail: "Starting the process." };
    case "running": {
      const l = signal && SIGNAL_LABEL[signal.kind];
      if (signal && l) return { label: l.label, detail: `The agent reported that it ${l.what} at ${clock(signal.at)}. This is its last report, not a live check.` };
      return { label: "Running", detail: "The process is running. What the agent is doing is not reported." };
    }
    case "exited":
      return { label: "Exited", detail: s.signal ? `Ended by signal ${s.signal}.` : `Exited with code ${s.code ?? "unknown"}.` };
    case "failed":
      return { label: "Failed", detail: s.message };
  }
}
