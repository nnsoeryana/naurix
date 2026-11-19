import { IconBot, IconPlus, IconStop } from "../../components/icons";
import { SHORTCUTS } from "../../shortcuts";
import { paneIds } from "../layout/layout";
import { useStore } from "../../state/store";
import { agentStatus } from "./status";

const dot: Record<string, string> = { Starting: "bg-warning", Running: "bg-teal", Exited: "bg-faint", Failed: "bg-danger" };

/** Agents section of the sidebar: launcher plus the agent sessions of the active workspace (AG-004). */
export function AgentSessions() {
  const wsId = useStore((s) => s.activeWorkspaceId);
  const space = useStore((s) => (s.activeWorkspaceId ? s.spaces[s.activeWorkspaceId] : undefined));
  const kinds = useStore((s) => s.paneKinds);
  const states = useStore((s) => s.paneState);
  const reveal = useStore((s) => s.revealPane);
  const close = useStore((s) => s.closePane);
  const launcher = useStore((s) => s.openLauncher);

  const items = (space?.tabs ?? []).flatMap((t) =>
    paneIds(t.layout)
      .filter((p) => kinds[p]?.type === "agent")
      .map((p) => ({ paneId: p, name: (kinds[p] as { name: string }).name, status: agentStatus(states[p]) })),
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
            <button className="flex min-w-0 flex-1 items-center gap-2 text-left" onClick={() => reveal(i.paneId)} title={i.status.detail}>
              <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${dot[i.status.label]}`} />
              <span className="truncate text-[13px] text-muted">{i.name}</span>
              <span className="ml-auto shrink-0 text-[10px] text-faint">{i.status.label}</span>
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
