import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "../../state/store";
import { approvalApi, type ApprovalRequest } from "../approvals/api";
import { workflowApi, type Run, type Workflow } from "./api";
import { addStep, canMarkDone, canSkip, draftOf, emptyDraft, removeStep, stepLabel, toggleDependency, type Draft } from "./model";

// The graph library is large, so it is fetched only when a run is shown as a graph (terminals never pay for it).
const RunGraph = lazy(() => import("./RunGraph"));

const outline = "rounded border border-line px-2 py-0.5 text-xs text-muted hover:bg-raised disabled:opacity-40";
const input = "rounded border border-line bg-transparent px-2 py-1 text-sm";
const tone: Record<string, string> = { ok: "text-teal", bad: "text-danger", warn: "text-warning", idle: "text-faint", live: "text-cloud" };

/** Workflows of the active workspace: saved sets of agent steps with dependencies, and their runs (ADR 0030). */
export function WorkflowsView() {
  const wsId = useStore((s) => s.activeWorkspaceId);
  const agents = useStore((s) => s.agents);
  const tick = useStore((s) => s.runTick);
  const openSession = useStore((s) => s.openSession);
  const [flows, setFlows] = useState<Workflow[]>([]);
  const [runs, setRuns] = useState<Run[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [review, setReview] = useState<{ workflow: Workflow; request: ApprovalRequest } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"graph" | "list">("graph");
  const reviewRef = useRef(review);
  reviewRef.current = review;
  const agentName = (id: string) => agents.find((a) => a.id === id)?.name ?? `${id} (not installed)`;

  const load = useCallback(async () => {
    if (!wsId) return;
    try {
      const [f, r] = await Promise.all([workflowApi.list(wsId), workflowApi.runs(wsId)]);
      setFlows(f);
      setRuns(r);
    } catch (e) {
      setError(String(e));
    }
  }, [wsId]);
  useEffect(() => void load(), [load, tick]);
  // A draft or a pending review belongs to the project it was started in.
  useEffect(() => {
    setDraft(null);
    if (reviewRef.current) void approvalApi.decide(reviewRef.current.request.id, { decision: "deny" }).catch(() => {});
    setReview(null);
    setError(null);
  }, [wsId]);
  useEffect(() => {
    // The event bus is lossy by design, so the view also re-reads when the window regains focus.
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [load]);

  if (!wsId) return <div className="p-6 text-sm text-muted">Open a project to use workflows.</div>;
  const act = (p: Promise<unknown>) => void p.then(() => (setError(null), load())).catch((e) => setError(String(e)));

  const save = async () => {
    if (!draft) return;
    try {
      await workflowApi.save(wsId, draft.id, draft.name, draft.steps);
      setDraft(null);
      setError(null);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const run = async (w: Workflow) => {
    try {
      const r = await workflowApi.run(w.id);
      if (r.kind === "required") setReview({ workflow: w, request: r.request });
      else (setReview(null), setError(null), await load());
    } catch (e) {
      setError(String(e));
    }
  };

  const approve = async () => {
    if (!review) return;
    try {
      await approvalApi.decide(review.request.id, { decision: "allow", scope: "once" });
      const r = await workflowApi.run(review.workflow.id);
      setReview(r.kind === "required" ? { workflow: review.workflow, request: r.request } : null);
      setError(null);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  const cancelReview = () => {
    if (review) void approvalApi.decide(review.request.id, { decision: "deny" }).catch(() => {});
    setReview(null);
  };

  const defaultAgent = agents[0]?.id ?? "";
  return (
    <div className="h-full overflow-auto p-6" data-workflows>
      <div className="mx-auto max-w-3xl space-y-5">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold">Workflows</h1>
          <button className="btn-primary ml-auto rounded-lg px-3 py-1 text-xs" disabled={!defaultAgent || !!draft} onClick={() => setDraft(emptyDraft(defaultAgent))}>New workflow</button>
        </div>
        <p className="text-xs text-muted">
          A workflow is a saved list of agents to run, with the order they wait for each other. Naurix stores only the names and the order, never prompts or output, and never types into an agent for you.
          You review every command once per run. A step is done when its program exits (0 means success) or when you press Mark done.
        </p>
        {error && <p className="rounded border border-danger/40 p-2 text-xs text-danger" role="alert">{error}</p>}

        {draft && <Editor draft={draft} setDraft={setDraft} agents={agents.map((a) => ({ id: a.id, name: a.name }))} defaultAgent={defaultAgent} save={save} cancel={() => (setDraft(null), setError(null))} />}

        {review && (
          <section className="space-y-2 rounded-lg border border-warning/50 p-3" data-run-review>
            <h2 className="text-sm font-semibold">Run “{review.workflow.name}”?</h2>
            <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded bg-raised/60 p-2 font-mono text-[11px]">{review.request.display}</pre>
            <div className="flex gap-2">
              <button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => void approve()} data-run-approve>Approve this run once and start</button>
              <button className={outline} onClick={cancelReview}>Cancel</button>
            </div>
          </section>
        )}

        <section className="space-y-1.5">
          {flows.length === 0 && !draft && <p className="text-sm text-faint">No workflows yet.</p>}
          {flows.map((w) => {
            const active = runs.some((r) => r.workflowId === w.id && r.state === "running");
            return (
              <div key={w.id} className="flex items-center gap-3 rounded-lg border border-line px-3 py-2" data-workflow>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-cloud">{w.name}</div>
                  <div className="truncate text-[11px] text-faint">{w.steps.map((s) => s.name).join(" → ")}</div>
                </div>
                <button className="btn-primary rounded-lg px-3 py-1 text-xs disabled:opacity-40" disabled={active || !!review} onClick={() => void run(w)} title={active ? "A run is already active" : "Review and run"} data-workflow-run>Run</button>
                <button className={outline} disabled={active} onClick={() => setDraft(draftOf(w))}>Edit</button>
                <button className={outline} disabled={active} onClick={() => act(workflowApi.remove(w.id))}>Delete</button>
              </div>
            );
          })}
        </section>

        <section className="space-y-3">
          <div className="flex items-center gap-2">
            <h2 className="text-sm font-semibold">Runs</h2>
            <span className="ml-auto flex overflow-hidden rounded border border-line text-xs" role="group" aria-label="Show runs as">
              {(["graph", "list"] as const).map((m) => (
                <button key={m} className={`px-2 py-0.5 ${mode === m ? "bg-raised text-cloud" : "text-muted hover:bg-raised/60"}`} aria-pressed={mode === m} onClick={() => setMode(m)} data-run-mode={m}>{m === "graph" ? "Graph" : "List"}</button>
              ))}
            </span>
          </div>
          {runs.length === 0 && <p className="text-sm text-faint">Nothing has run yet.</p>}
          {runs.map((r, i) => (
            <details key={r.id} open={r.state === "running" || i === 0} className="rounded-lg border border-line px-3 py-2" data-run={r.state}>
              <summary className="flex cursor-pointer items-center gap-2 text-sm">
                <span className="text-cloud">{r.name}</span>
                <span className={`text-xs ${r.state === "succeeded" ? "text-teal" : r.state === "running" ? "text-cloud" : "text-warning"}`} data-run-state>{r.state}</span>
                <span className="ml-auto text-[11px] text-faint">{r.createdAt}</span>
              </summary>
              {mode === "graph" && (
                <div className="mt-2">
                  <Suspense fallback={<p className="text-xs text-faint">Loading graph…</p>}>
                    <RunGraph run={r} agentName={agentName} onOpen={(sid) => void openSession(sid)} onDone={(stepId) => act(workflowApi.markDone(r.id, stepId))} onSkip={(stepId) => act(workflowApi.skip(r.id, stepId))} />
                  </Suspense>
                </div>
              )}
              {mode === "list" && (
              <ul className="mt-2 space-y-1.5">
                {r.steps.map((s) => {
                  const l = stepLabel(r, s);
                  const sid = r.sessions[s.id];
                  return (
                    <li key={s.id} className="flex items-center gap-2" data-run-step={s.state}>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-[13px] text-cloud">{s.name} <span className="text-[11px] text-faint">· {agentName(s.agentId)}</span></span>
                        <span className={`block text-[11px] ${tone[l.tone]}`} data-step-label>{l.text}</span>
                      </span>
                      {sid !== undefined && <button className={outline} onClick={() => void openSession(sid)}>Open terminal</button>}
                      {canMarkDone(r, s) && <button className={outline} onClick={() => act(workflowApi.markDone(r.id, s.id))} title="Finishes this step for the workflow. The agent keeps running.">Mark done</button>}
                      {canSkip(r, s) && <button className={outline} onClick={() => act(workflowApi.skip(r.id, s.id))} title="Go on without this step.">Skip</button>}
                    </li>
                  );
                })}
              </ul>
              )}
              {r.state === "running" && (
                <div className="mt-2 flex items-center gap-3">
                  <button className={outline} onClick={() => act(workflowApi.cancel(r.id))}>Cancel run</button>
                  <span className="text-[11px] text-faint">Cancelling starts nothing new and leaves running agents open.</span>
                </div>
              )}
            </details>
          ))}
        </section>
      </div>
    </div>
  );
}

function Editor({ draft, setDraft, agents, defaultAgent, save, cancel }: { draft: Draft; setDraft: (d: Draft) => void; agents: { id: string; name: string }[]; defaultAgent: string; save: () => Promise<void>; cancel: () => void }) {
  return (
    <section className="space-y-3 rounded-lg border border-line p-3" data-workflow-editor>
      <input aria-label="Workflow name" className={`${input} w-full`} placeholder="Name, for example “Build, then review”" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
      <ol className="space-y-2">
        {draft.steps.map((s, i) => (
          <li key={i} className="space-y-1 rounded border border-line/60 p-2" data-step-editor>
            <div className="flex items-center gap-2">
              <span className="w-5 text-xs text-faint">{i + 1}.</span>
              <input aria-label={`Step ${i + 1} name`} className={`${input} min-w-0 flex-1`} value={s.name} onChange={(e) => setDraft({ ...draft, steps: draft.steps.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)) })} />
              <select aria-label={`Step ${i + 1} agent`} className={input} value={s.agentId} onChange={(e) => setDraft({ ...draft, steps: draft.steps.map((x, j) => (j === i ? { ...x, agentId: e.target.value } : x)) })}>
                {!agents.some((a) => a.id === s.agentId) && <option value={s.agentId}>{s.agentId} (not installed)</option>}
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
              </select>
              <button className={outline} disabled={draft.steps.length === 1} onClick={() => setDraft(removeStep(draft, i))} aria-label={`Remove step ${i + 1}`}>Remove</button>
            </div>
            {i > 0 && (
              <div className="ml-7 flex flex-wrap items-center gap-3 text-[11px] text-muted">
                <span>Starts after:</span>
                {draft.steps.slice(0, i).map((p, j) => (
                  <label key={j} className="flex items-center gap-1">
                    <input type="checkbox" checked={s.dependsOn.includes(j)} onChange={() => setDraft(toggleDependency(draft, i, j))} /> {j + 1}. {p.name}
                  </label>
                ))}
                {s.dependsOn.length === 0 && <span className="text-faint">(nothing: starts with the run)</span>}
              </div>
            )}
          </li>
        ))}
      </ol>
      <div className="flex items-center gap-2">
        <button className={outline} onClick={() => setDraft(addStep(draft, defaultAgent))} disabled={draft.steps.length >= 24}>Add step</button>
        <button className="btn-primary ml-auto rounded-lg px-3 py-1 text-xs" onClick={() => void save()}>Save</button>
        <button className={outline} onClick={cancel}>Cancel</button>
      </div>
      <p className="text-[11px] text-faint">A step only names an agent. To give an agent fixed arguments, add it as a custom agent in Settings. You type prompts yourself in the agent&apos;s terminal.</p>
    </section>
  );
}
