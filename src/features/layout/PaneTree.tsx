import { useRef } from "react";
import { TerminalView } from "../terminal/TerminalView";
import { useStore, type Tab } from "../../state/store";
import { computeGeometry } from "./layout";

type Ctx = { wsId: string; tab: Tab; visible: boolean; multi: boolean };

const pct = (f: number) => `${f * 100}%`;

/**
 * Panes are rendered as a flat list keyed by pane id and positioned absolutely from the layout
 * geometry. Splitting or closing a sibling therefore never remounts (and never kills) a terminal.
 */
export function PaneTree({ wsId, tab, visible }: { wsId: string; tab: Tab; visible: boolean }) {
  const setRatio = useStore((s) => s.setRatio);
  const box = useRef<HTMLDivElement>(null);
  const { panes, splits } = computeGeometry(tab.layout);
  const ctx: Ctx = { wsId, tab, visible, multi: tab.layout.type === "split" };

  return (
    <div ref={box} className={visible ? "absolute inset-0" : "hidden"}>
      {Object.entries(panes).map(([paneId, r]) => (
        <div key={paneId} className="absolute" style={{ left: pct(r.x), top: pct(r.y), width: pct(r.w), height: pct(r.h) }}>
          <Pane paneId={paneId} ctx={ctx} />
        </div>
      ))}
      {splits.map((sp) => {
        const row = sp.dir === "row";
        const pos = row ? { left: `calc(${pct(sp.rect.x + sp.rect.w * sp.ratio)} - 2px)`, top: pct(sp.rect.y), width: 4, height: pct(sp.rect.h) } : { left: pct(sp.rect.x), top: `calc(${pct(sp.rect.y + sp.rect.h * sp.ratio)} - 2px)`, width: pct(sp.rect.w), height: 4 };
        const onDrag = (e: React.PointerEvent) => {
          if (e.buttons !== 1 || !box.current) return;
          const b = box.current.getBoundingClientRect();
          const ratio = row ? (e.clientX - b.left - sp.rect.x * b.width) / (sp.rect.w * b.width) : (e.clientY - b.top - sp.rect.y * b.height) / (sp.rect.h * b.height);
          setRatio(tab.id, sp.id, ratio);
        };
        return (
          <div
            key={sp.id}
            role="separator"
            aria-orientation={row ? "vertical" : "horizontal"}
            className={`absolute z-10 bg-raised hover:bg-purple ${row ? "cursor-col-resize" : "cursor-row-resize"}`}
            style={pos}
            onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)}
            onPointerMove={onDrag}
          />
        );
      })}
    </div>
  );
}

function Pane({ paneId, ctx }: { paneId: string; ctx: Ctx }) {
  const gen = useStore((s) => s.paneGen[paneId] ?? 0);
  const state = useStore((s) => s.paneState[paneId]);
  const setPaneState = useStore((s) => s.setPaneState);
  const focusPane = useStore((s) => s.focusPane);
  const restart = useStore((s) => s.restartPane);
  const closePane = useStore((s) => s.closePane);
  const kind = useStore((s) => s.paneKinds[paneId]);
  const isActive = ctx.tab.activePaneId === paneId;
  const ended = state?.kind === "exited" || state?.kind === "failed";

  return (
    <div className={`relative h-full w-full ${ctx.multi && isActive && ctx.visible ? "ring-1 ring-inset ring-teal/70" : ""}`} onMouseDownCapture={() => focusPane(paneId)}>
      <TerminalView
        key={`${paneId}:${gen}`}
        workspaceId={ctx.wsId}
        paneId={paneId}
        title={kind?.type === "agent" ? kind.name : ctx.tab.title}
        visible={ctx.visible}
        focused={isActive && ctx.visible}
        agent={kind?.type === "agent" ? { id: kind.agentId } : undefined}
        atRepoRoot={kind?.type === "shell" ? kind.atRepoRoot : undefined}
        attachTo={kind?.sessionId}
        onState={(s) => setPaneState(paneId, s)}
      />
      {ended && (
        <div className="absolute inset-x-0 bottom-0 flex items-center gap-3 bg-surface/95 px-3 py-1 text-xs text-cloud/80">
          <span>{state.kind === "failed" ? `Failed to start: ${state.message}` : state.signal ? `Process ended (signal ${state.signal})` : `Process exited (code ${state.code ?? "?"})`}</span>
          <button className="rounded bg-raised px-2 py-0.5 hover:bg-line" onClick={() => restart(paneId)}>{kind?.type === "agent" ? "Launch again" : "Restart"}</button>
          <button className="rounded bg-raised px-2 py-0.5 hover:bg-line" onClick={() => closePane(paneId)}>Close</button>
        </div>
      )}
    </div>
  );
}
