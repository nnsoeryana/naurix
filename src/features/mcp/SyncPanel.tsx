import { useEffect, useState } from "react";
import { useStore } from "../../state/store";
import { mcpApi, type SyncAgent, type SyncPlan, type SyncRun } from "./api";

const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";
const tone: Record<string, string> = { add: "text-teal", update: "text-purple", remove: "text-warning", unchanged: "text-faint", conflict: "text-danger" };

/** Preview, apply and undo writes into agent config files. Only agents verified in M6 are enabled. */
export function SyncPanel({ workspaceId, workspaceName, agents, onError }: { workspaceId: string | null; workspaceName: string | null; agents: SyncAgent[]; onError: (e: unknown) => void }) {
  const [plan, setPlan] = useState<SyncPlan | null>(null);
  const [runs, setRuns] = useState<Record<string, SyncRun[]>>({});
  const [last, setLast] = useState<string | null>(null);

  useEffect(() => {
    for (const a of agents.filter((x) => x.supported)) mcpApi.syncRuns(a.agentId).then((r) => setRuns((m) => ({ ...m, [a.agentId]: r }))).catch(onError);
  }, [agents, onError]);

  const preview = (agentId: string) => workspaceId && mcpApi.syncPlan(workspaceId, agentId).then((p) => (setLast(null), setPlan(p))).catch(onError);
  const writes = plan?.changes.some((c) => c.action === "add" || c.action === "update" || c.action === "remove") ?? false;

  const apply = () => {
    if (!plan || !workspaceId) return;
    const p = plan;
    useStore.getState().setDialog({
      kind: "confirm",
      title: "Write to the agent config?",
      message: `Naurix will back up ${p.file} first and change only the entries listed. You can undo this afterwards.`,
      confirmLabel: "Apply",
      onConfirm: () =>
        void mcpApi
          .syncApply(workspaceId, p.agent_id, p.plan_hash)
          .then((run) => {
            setLast(`Applied: ${run.summary}.`);
            setPlan(null);
            return mcpApi.syncRuns(p.agent_id).then((r) => setRuns((m) => ({ ...m, [p.agent_id]: r })));
          })
          .catch(onError),
    });
  };

  const undo = (run: SyncRun) =>
    void mcpApi
      .syncRollback(run.id)
      .then(() => {
        setLast(`Undone: ${run.summary}.`);
        return mcpApi.syncRuns(run.agent_id).then((r) => setRuns((m) => ({ ...m, [run.agent_id]: r })));
      })
      .catch(onError);

  return (
    <section className="mt-10" aria-label="Sync to agents">
      <h2 className="text-lg font-semibold">Sync to agents</h2>
      <p className="mt-1 text-sm text-muted">Write your approved, enabled servers into an agent&apos;s config. Always a preview first, with a backup and undo. Only agents whose format was verified against the real CLI are available{workspaceName ? <> (project: <span className="text-cloud">{workspaceName}</span>)</> : null}.</p>
      <ul className="mt-4 space-y-2">
        {agents.map((a) => (
          <li key={a.agentId} className="rounded-lg border border-line bg-surface/60 p-3">
            <div className="flex items-center gap-3">
              <div className="min-w-0 flex-1">
                <div className="text-sm">{a.name}</div>
                <p className="text-xs text-faint">{a.note}</p>
              </div>
              <button className={outline} disabled={!a.supported || !workspaceId} onClick={() => void preview(a.agentId)} title={a.supported ? "" : "Not verified yet"}>Preview changes…</button>
            </div>
            {(runs[a.agentId] ?? []).filter((r) => !r.rolled_back_at).slice(0, 1).map((r) => (
              <div key={r.id} className="mt-2 flex items-center gap-2 border-t border-line pt-2 text-xs text-muted">
                <span>Last sync {r.created_at.replace("T", " ").replace("Z", " UTC")}: {r.summary}.</span>
                <button className={`${outline} ml-auto`} onClick={() => undo(r)}>Undo</button>
              </div>
            ))}
          </li>
        ))}
      </ul>
      {last && <p className="mt-3 text-sm text-teal">{last}</p>}

      {plan && (
        <div className="mt-4 rounded-xl border border-line bg-surface p-4" data-sync-plan>
          <h3 className="text-sm font-semibold">Plan for {plan.agent_id}</h3>
          <p className="mt-1 break-all font-mono text-xs text-faint">{plan.file}{plan.file_exists ? "" : " (will be created)"}</p>
          {plan.blocked && <p className="mt-3 rounded border border-warning/40 bg-warning/10 p-2 text-xs text-warning">{plan.blocked}</p>}
          <ul className="mt-3 space-y-1 text-xs">
            {plan.changes.length === 0 && <li className="text-faint">No approved, enabled servers to write, and nothing of yours to remove.</li>}
            {plan.changes.map((c) => (
              <li key={c.entry_key} className="flex gap-2"><span className={`w-20 shrink-0 font-semibold uppercase ${tone[c.action]}`}>{c.action}</span><span className="font-mono">{c.entry_key}</span><span className="text-faint">{c.detail}</span></li>
            ))}
          </ul>
          {plan.preview && plan.preview !== "{}" && (
            <details className="mt-3 text-xs" open={!!plan.blocked}>
              <summary className="cursor-pointer text-muted">Entries Naurix manages</summary>
              <pre className="mt-2 max-h-56 overflow-auto rounded bg-midnight p-2 font-mono text-[11px] text-cloud/90">{plan.preview}</pre>
            </details>
          )}
          <ul className="mt-3 list-disc pl-4 text-[11px] text-faint">{plan.notes.map((n) => <li key={n}>{n}</li>)}</ul>
          <div className="mt-4 flex justify-end gap-2">
            <button className={outline} onClick={() => setPlan(null)}>Close</button>
            <button className="btn-primary rounded-lg px-3 py-1 text-xs disabled:opacity-40" disabled={!writes || !!plan.blocked} onClick={apply}>Apply…</button>
          </div>
        </div>
      )}
    </section>
  );
}
