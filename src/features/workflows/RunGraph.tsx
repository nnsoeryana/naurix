import "@xyflow/react/dist/style.css";
import { Controls, Handle, Position, ReactFlow, type Edge, type Node, type NodeProps } from "@xyflow/react";
import { createContext, useContext, useMemo } from "react";
import { useStore } from "../../state/store";
import type { Run } from "./api";
import { edgesOf, graphHeight, layoutSteps, NODE_H, NODE_W } from "./graph";
import { canMarkDone, canSkip, isBlocked, stepLabel } from "./model";

/** The graph of one run (ADR 0030): a read-only picture of the same state the list shows. Loaded on demand. */

const border: Record<string, string> = { ok: "border-teal/60", bad: "border-danger/60", warn: "border-warning/60", idle: "border-line", live: "border-cloud" };
// The state is written out in readable text; its colour is only a second cue (a dot and the border), because
// the accent text colours do not reach 4.5:1 on the light theme.
const dot: Record<string, string> = { ok: "bg-teal", bad: "bg-danger", warn: "bg-warning", idle: "bg-faint", live: "bg-cloud" };
const btn = "nodrag nopan rounded border border-line px-1.5 py-0.5 text-[10px] text-muted hover:bg-raised";

/**
 * The run changes often, but React Flow wants stable node objects (new objects make it forget the
 * measured handles and the edges disappear). Nodes therefore carry only an id; the card reads the live
 * run and the handlers from this context.
 */
type Live = {
  run: Run;
  agentName: (id: string) => string;
  onOpen: (session: number) => void;
  onDone: (stepId: string) => void;
  onSkip: (stepId: string) => void;
};
const LiveRun = createContext<Live | null>(null);
type StepNode = Node<{ stepId: string }, "step">;

function StepCard({ data }: NodeProps<StepNode>) {
  const live = useContext(LiveRun)!;
  const { run, agentName, onOpen, onDone, onSkip } = live;
  const step = run.steps.find((x) => x.id === data.stepId);
  if (!step) return null;
  const session = run.sessions[step.id];
  const l = stepLabel(run, step);
  return (
    <div className={`rounded-lg border bg-raised px-2.5 py-2 ${border[l.tone]}`} style={{ width: NODE_W, height: NODE_H }} data-graph-step={step.state} aria-label={`${step.name}: ${l.text}`}>
      <Handle type="target" position={Position.Left} isConnectable={false} className="!h-1.5 !w-1.5 !border-0 !bg-faint" />
      <div className="truncate text-[13px] text-cloud" title={step.name}>{step.name}</div>
      <div className="truncate text-[10px] text-faint">{agentName(step.agentId)}</div>
      <div className="mt-0.5 flex items-start gap-1.5 text-[11px] leading-tight text-cloud">
        <span className={`mt-1 h-1.5 w-1.5 shrink-0 rounded-full ${dot[l.tone]}`} aria-hidden="true" />
        <span className="line-clamp-2" data-graph-label>{l.text}</span>
      </div>
      <div className="mt-1 flex gap-1">
        {session !== undefined && <button className={btn} onClick={() => onOpen(session)}>Open terminal</button>}
        {canMarkDone(run, step) && <button className={btn} onClick={() => onDone(step.id)}>Mark done</button>}
        {canSkip(run, step) && <button className={btn} onClick={() => onSkip(step.id)}>Skip</button>}
      </div>
      <Handle type="source" position={Position.Right} isConnectable={false} className="!h-1.5 !w-1.5 !border-0 !bg-faint" />
    </div>
  );
}

const nodeTypes = { step: StepCard };

export default function RunGraph({ run, agentName, onOpen, onDone, onSkip }: { run: Run; agentName: (id: string) => string; onOpen: (session: number) => void; onDone: (stepId: string) => void; onSkip: (stepId: string) => void }) {
  const dark = useStore((s) => s.theme.kind !== "light");
  // Only a change in the shape of the run (its steps and dependencies) makes new nodes.
  const shape = run.steps.map((x) => `${x.id}:${x.name}:${x.dependsOn.join(",")}`).join("|");
  const { nodes, height } = useMemo(() => {
    const pos = layoutSteps(run.steps);
    const nodes: StepNode[] = run.steps.map((step) => ({
      id: step.id,
      type: "step",
      position: pos.get(step.id) ?? { x: 0, y: 0 },
      data: { stepId: step.id },
      // Read when the node is focused; the state is in its text. Stable across state changes, so React Flow keeps its measurements.
      ariaLabel: `Step ${step.name}`,
      draggable: false,
      connectable: false,
    }));
    return { nodes, height: graphHeight(run.steps) };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape]);
  const edges: Edge[] = edgesOf(run, (s) => isBlocked(run, s)).map((e) => ({
    id: e.id,
    source: e.source,
    target: e.target,
    animated: e.animated,
    ariaLabel: `${run.steps.find((x) => x.id === e.source)?.name ?? "A step"} comes before ${run.steps.find((x) => x.id === e.target)?.name ?? "a step"}`,
    style: e.blocked ? { strokeDasharray: "4 4", opacity: 0.5 } : undefined,
  }));
  const live = useMemo(() => ({ run, agentName, onOpen, onDone, onSkip }), [run, agentName, onOpen, onDone, onSkip]);

  return (
    <div style={{ height, ["--xy-edge-stroke" as string]: "var(--color-muted)", ["--xy-edge-stroke-selected" as string]: "var(--color-purple)" }} className="rounded border border-line/60" data-run-graph>
      <LiveRun.Provider value={live}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        className={dark ? "dark" : "light"}
        fitView
        fitViewOptions={{ padding: 0.15 }}
        nodesDraggable={false}
        nodesConnectable={false}
        edgesFocusable={false}
        zoomOnScroll={false}
        panOnScroll={false}
        preventScrolling={false}
        minZoom={0.4}
        maxZoom={1.5}
        aria-label={`Graph of the run ${run.name}`}
      >
        <Controls showInteractive={false} />
      </ReactFlow>
      </LiveRun.Provider>
    </div>
  );
}
