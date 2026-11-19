import type { TerminalState } from "../terminal/TerminalView";

/**
 * Honest agent status (PRD AG-005/AG-009). With the generic PTY adapter the only thing we know is
 * the PROCESS state. We never claim "waiting for approval", "working" or "completed": that needs a
 * reliable adapter signal, which no agent provides yet.
 */
export type AgentStatus = { label: "Starting" | "Running" | "Exited" | "Failed"; detail: string };

export function agentStatus(s: TerminalState | undefined): AgentStatus {
  switch (s?.kind) {
    case undefined:
    case "starting":
      return { label: "Starting", detail: "Starting the process." };
    case "running":
      return { label: "Running", detail: "The process is running. What the agent is doing is not reported." };
    case "exited":
      return { label: "Exited", detail: s.signal ? `Ended by signal ${s.signal}.` : `Exited with code ${s.code ?? "unknown"}.` };
    case "failed":
      return { label: "Failed", detail: s.message };
  }
}
