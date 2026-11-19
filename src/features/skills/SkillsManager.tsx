import { useCallback, useEffect, useState } from "react";
import { Modal } from "../../components/Modal";
import { useStore } from "../../state/store";
import { agentLabel, skillApi, sourceLine, type SkillAgent, type SkillView, type UpdatePlan } from "./api";
import { InstallDialog } from "./InstallDialog";
import { ManageDialog } from "./ManageDialog";

const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";
const chipTone: Record<string, string> = { ok: "border-teal/40 text-teal", needs_fix: "border-warning/50 text-warning", unknown: "border-line text-faint" };

/** Agent Skills screen (M8): install from Git or a folder, enable per agent, update, revert, uninstall. */
export function SkillsManager() {
  const workspaceId = useStore((s) => s.activeWorkspaceId);
  const workspaceName = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId)?.name ?? null);
  const setNotice = useStore((s) => s.setNotice);
  const [views, setViews] = useState<SkillView[]>([]);
  const [agents, setAgents] = useState<SkillAgent[]>([]);
  const [dialog, setDialog] = useState<null | { kind: "install" } | { kind: "manage"; id: string } | { kind: "update"; plan: UpdatePlan; name: string }>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  const fail = useCallback((e: unknown) => setNotice(String(e)), [setNotice]);
  const refresh = useCallback(() => skillApi.list(workspaceId).then(setViews).catch(fail), [workspaceId, fail]);
  useEffect(() => {
    void refresh();
    skillApi.agents().then(setAgents).catch(fail);
  }, [refresh, fail]);

  const checkUpdate = (v: SkillView) => {
    setBusy(v.skill.id);
    setInfo(null);
    skillApi
      .checkUpdate(v.skill.id)
      .then((plan) => (plan.up_to_date ? setInfo(`“${v.skill.name}” is up to date.`) : setDialog({ kind: "update", plan, name: v.skill.name })))
      .catch(fail)
      .finally(() => setBusy(null));
  };

  const uninstall = (v: SkillView) =>
    useStore.getState().setDialog({
      kind: "confirm",
      title: `Uninstall “${v.skill.name}”?`,
      message: "Naurix removes its stored copy and the links it created. A link you replaced yourself is left alone. The original source folder or repository is not touched.",
      confirmLabel: "Uninstall",
      danger: true,
      onConfirm: () =>
        void skillApi
          .uninstall(v.skill.id)
          .then((r) => {
            setInfo(r.left_alone.length ? `Uninstalled. Left alone because you changed them: ${r.left_alone.join(", ")}` : "Uninstalled.");
            return refresh();
          })
          .catch(fail),
    });

  const managing = dialog?.kind === "manage" ? views.find((v) => v.skill.id === dialog.id) : undefined;

  return (
    <div className="absolute inset-0 overflow-auto bg-midnight">
      <div className="mx-auto max-w-3xl px-8 pb-16 pt-10">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold">Agent Skills</h1>
            <p className="mt-1 max-w-xl text-sm text-muted">Reusable instructions for your agents. Each skill is stored once, then linked into the agents you choose. Skills are untrusted: Naurix never runs anything inside them.</p>
          </div>
          <button className="btn-primary shrink-0 rounded-lg px-3 py-1 text-xs" onClick={() => setDialog({ kind: "install" })}>Install skills…</button>
        </div>
        {info && <p className="mt-4 text-sm text-teal">{info}</p>}

        <ul className="mt-6 space-y-3">
          {views.length === 0 && <li className="rounded-lg border border-dashed border-line p-6 text-center text-sm text-faint">No skills installed. Install from a Git repository or a folder on this Mac.</li>}
          {views.map((v) => {
            const on = v.links.filter((l) => l.health === "ok");
            const broken = v.links.filter((l) => l.health !== "ok");
            return (
              <li key={v.skill.id} className="rounded-xl border border-line bg-surface/60 p-4" data-skill={v.skill.name}>
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{v.skill.name}</span>
                  <span className="rounded bg-raised px-1.5 text-[10px] uppercase tracking-wide text-muted">{v.skill.source_kind}</span>
                  {v.skill.flags.some((f) => ["scripts", "shell_injection", "allows_shell_tools", "defines_hooks"].includes(f.kind)) && <span className="rounded-full border border-warning/50 px-2 text-[11px] text-warning" title={v.skill.flags.map((f) => f.detail).join("\n")}>Contains scripts or commands</span>}
                </div>
                {v.skill.description && <p className="mt-1 text-sm text-muted">{v.skill.description}</p>}
                <p className="mt-1 break-all font-mono text-[11px] text-faint">{sourceLine(v.skill)}</p>
                <ul className="mt-2 flex flex-wrap gap-1.5 text-[11px]">
                  {v.compat.filter((c) => c.status !== "unsupported").map((c) => (
                    <li key={c.agent_id} className={`rounded-full border px-2 ${chipTone[c.status]}`} title={c.note}>{agentLabel[c.agent_id] ?? c.agent_id}: {c.status === "ok" ? "compatible" : c.status === "needs_fix" ? "needs fixes" : "unknown"}</li>
                  ))}
                </ul>
                <p className="mt-2 text-xs text-muted">
                  {on.length === 0 ? "Not enabled for any agent." : `Enabled: ${on.map((l) => `${agentLabel[l.agent_id] ?? l.agent_id} (${l.scope === "user" ? "all projects" : "project"})`).join(", ")}`}
                  {broken.length > 0 && <span className="text-warning"> · {broken.length} link(s) changed or missing</span>}
                </p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => setDialog({ kind: "manage", id: v.skill.id })}>Use with agents…</button>
                  <button className={outline} disabled={busy === v.skill.id} onClick={() => checkUpdate(v)}>{busy === v.skill.id ? "Checking…" : "Check for updates"}</button>
                  {v.skill.prev_rev_dir && <button className={outline} onClick={() => void skillApi.revert(v.skill.id).then(() => (setInfo("Switched to the other stored version."), refresh())).catch(fail)}>Switch to previous version</button>}
                  <button className={outline} onClick={() => uninstall(v)}>Uninstall</button>
                </div>
              </li>
            );
          })}
        </ul>
      </div>

      {dialog?.kind === "install" && <InstallDialog onClose={() => setDialog(null)} onDone={() => (setDialog(null), setInfo("Installed."), void refresh())} />}
      {managing && (
        <ManageDialog
          view={managing}
          agents={agents}
          workspaceName={workspaceName}
          onClose={() => setDialog(null)}
          onToggle={(agentId, scope, enabled) => void skillApi.setLink(managing.skill.id, agentId, scope, enabled, workspaceId).then(setViews).catch(fail)}
        />
      )}
      {dialog?.kind === "update" && (
        <Modal title={`Update “${dialog.name}”`} onClose={() => (void skillApi.discardStaging(dialog.plan.staging_id), setDialog(null))} wide>
          <ul className="space-y-1 font-mono text-xs">
            {dialog.plan.added.map((f) => <li key={`a${f}`} className="text-teal">+ {f}</li>)}
            {dialog.plan.changed.map((f) => <li key={`c${f}`} className="text-purple">~ {f}</li>)}
            {dialog.plan.removed.map((f) => <li key={`r${f}`} className="text-warning">− {f}</li>)}
          </ul>
          {dialog.plan.new_flags.length > 0 && (
            <div className="mt-3 space-y-1">{dialog.plan.new_flags.map((f) => <p key={f.kind} className="rounded border border-warning/40 bg-warning/10 p-2 text-xs text-warning">New in this version: {f.detail}</p>)}</div>
          )}
          <p className="mt-3 text-[11px] text-faint">The current version is kept so you can switch back. Links keep working.</p>
          <div className="mt-4 flex justify-end gap-2">
            <button className="rounded bg-raised px-3 py-1 text-sm hover:bg-line" onClick={() => (void skillApi.discardStaging(dialog.plan.staging_id), setDialog(null))}>Cancel</button>
            <button className="btn-primary rounded px-3 py-1 text-sm" onClick={() => void skillApi.applyUpdate(dialog.plan).then(() => (setDialog(null), setInfo("Updated."), refresh())).catch(fail)}>Apply update</button>
          </div>
        </Modal>
      )}
    </div>
  );
}
