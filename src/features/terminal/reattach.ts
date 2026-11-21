import type { PaneKind } from "../../state/store";
import type { LiveSession } from "./api";

export type ReattachItem = { workspaceId: string; sessionId: number; title: string; kind: PaneKind };

/**
 * After a reload the UI has forgotten its panes but Core still owns the sessions (ADR 0026). Each
 * session of a known workspace comes back as one tab, oldest first. Split layout is not restored
 * (ADR 0010: layout is UI state). Sessions of workspaces we no longer list are skipped.
 */
export function planReattach(sessions: LiveSession[], knownWorkspaceIds: ReadonlySet<string>): ReattachItem[] {
  return [...sessions]
    .filter((s) => knownWorkspaceIds.has(s.workspaceId))
    .sort((a, b) => a.id - b.id)
    .map((s) => ({
      workspaceId: s.workspaceId,
      sessionId: s.id,
      title: s.title,
      kind: s.kind === "agent" ? { type: "agent", agentId: s.adapterId ?? "", name: s.title, reviewed: true, sessionId: s.id } : { type: "shell", sessionId: s.id },
    }));
}
