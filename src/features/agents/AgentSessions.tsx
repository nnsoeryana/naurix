import { IconBot, IconPlus, IconStop } from "../../components/icons";
import { SHORTCUTS } from "../../shortcuts";
import { paneIds } from "../layout/layout";
import { useStore } from "../../state/store";
import { sessionRegistry } from "../terminal/registry";
import { agentStatus } from "./status";

const dot: Record<string, string> = { Starting: "bg-warning", Running: "bg-teal", Exited: "bg-faint", Failed: "bg-danger", Working: "bg-teal", "Waiting for approval": "bg-warning", "Waiting for input": "bg-faint", "Turn finished": "bg-faint" };

/** Only live states animate (transform/opacity only; off under prefers-reduced-motion). */
const live = new Set(["Starting", "Running", "Working", "Waiting for approval"]);

/** Agents section of the sidebar: launcher plus the agent sessions of the active workspace (AG-004). */
export function AgentSessions() {
  const wsId = useStore((s) => s.activeWorkspaceId);
  const space = useStore((s) => (s.activeWorkspaceId ? s.spaces[s.activeWorkspaceId] : undefined));
  const kinds = useStore((s) => s.paneKinds);
  const states = useStore((s) => s.paneState);
  const signals = useStore((s) => s.sessionStatus);
  const reveal = useStore((s) => s.revealPane);
  const close = useStore((s) => s.closePane);
  const launcher = useStore((s) => s.openLauncher);

  const items = (space?.tabs ?? []).flatMap((t) =>
    paneIds(t.layout)
      .filter((p) => kinds[p]?.type === "agent")
      .map((p) => ({ paneId: p, name: (kinds[p] as { name: string }).name, branch: (kinds[p] as { branch?: string }).branch, status: agentStatus(states[p], signals[sessionRegistry.get(p) ?? (kinds[p] as { sessionId?: number }).sessionId ?? -1]) })),
  );

  return (
    <>
      <div className="flex items-center">
        <div className="flex flex-1 items-center gap-2.5 px-2.5 py-1.5 text-sm text-muted">
          <IconBot /> Agents
        </div>
        <button className="rounded p-1 text-faint hover:bg-raised hover:text-cloud disabled:opacity-40" disabled={!wsId} onClick={() => launcher(true)} title={`Launch AI agent (${SHORTCUTS.launchAgent.label})`} aria-label="Launch AI agent">
          <IconPlus />
        </button>
      </div>
      <ul className="mb-1 ml-3 border-l border-line pl-2">
        {items.length === 0 && <li className="px-2 py-1 text-xs text-faint">No agents running</li>}
        {items.map((i) => (
          <li key={i.paneId} className="group flex items-center gap-2 rounded-lg px-2 py-1 hover:bg-raised/60">
            <button className="flex min-w-0 flex-1 items-center gap-2 text-left" onClick={() => reveal(i.paneId)} title={i.status.detail} data-agent-detail>
              <span className="relative flex h-1.5 w-1.5 shrink-0">
                {live.has(i.status.label) && <span className={`absolute inline-flex h-full w-full rounded-full opacity-75 motion-safe:animate-ping ${dot[i.status.label]}`} />}
                <span className={`relative inline-flex h-1.5 w-1.5 rounded-full ${dot[i.status.label]}`} />
              </span>
              <span className="truncate text-[13px] text-muted">{i.name}</span>
              {i.branch && <span className="max-w-[7rem] shrink truncate rounded bg-raised px-1 font-mono text-[10px] text-faint" title={`Running in its own worktree on ${i.branch}`} data-worktree-branch>⎇ {i.branch.replace(/^naurix\//, "")}</span>}
              <span className="ml-auto shrink-0 text-[10px] text-faint" data-agent-status>{i.status.label}</span>
            </button>
            <button aria-label={`Stop ${i.name}`} className="invisible rounded p-1 text-faint hover:bg-line hover:text-cloud group-hover:visible" onClick={() => close(i.paneId)} title="Stop and close">
              <IconStop />
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}
