import type { PaneKind } from "../../state/store";
import type { TerminalState } from "../terminal/TerminalView";

export type InsertTarget = { ok: true; paneId: string } | { ok: false; reason: string };

/**
 * Where "Insert into agent terminal" may type (ADR 0031 §6): only the active pane, and only when it is a
 * running agent session. A plain shell is refused, because pasted newlines could run commands there.
 */
export function insertTarget(paneId: string | undefined, kinds: Record<string, PaneKind>, states: Record<string, TerminalState>): InsertTarget {
  if (!paneId) return { ok: false, reason: "Open an agent first: there is no active terminal." };
  const kind = kinds[paneId];
  if (kind?.type !== "agent") return { ok: false, reason: "The active terminal is not an agent. Notes are only inserted into agents, never into a shell." };
  if (states[paneId]?.kind !== "running") return { ok: false, reason: "The active agent is not running." };
  return { ok: true, paneId };
}

/** The text typed into the agent: the note as written, never a trailing newline, so nothing is sent. */
export function insertText(n: { title: string; body: string }): string {
  return `${n.title}\n${n.body}`.replace(/\s+$/, "");
}
