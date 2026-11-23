import { useEffect, useState } from "react";
import { useStore } from "../../state/store";
import { approvalApi, scopeLabel, type ApprovalRequest, type ApprovalScope } from "../approvals/api";
import { agentApi, type AgentInfo, type CustomAgentInput } from "./api";
import { Capabilities } from "./Capabilities";

/** `request` is set when the backend asked for approval: the review then shows ITS text, not anything the UI built. */
type Mode = { kind: "list" } | { kind: "review"; id: string; request?: ApprovalRequest } | { kind: "edit"; draft: Draft };
type Draft = { id?: string; name: string; executable: string; args: string; env: string };

const field = "w-full rounded border border-line bg-midnight px-2 py-1 text-sm outline-none focus:border-purple";
const btn = "rounded px-3 py-1 text-sm";

export function AgentLauncher() {
  const open = useStore((s) => s.launcherOpen);
  if (!open) return null;
  return <Launcher />;
}

function Launcher() {
  const agents = useStore((s) => s.agents);
  const setAgents = useStore((s) => s.setAgents);
  const close = () => useStore.getState().openLauncher(false);
  const workspace = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId));
  const [mode, setMode] = useState<Mode>({ kind: "list" });
  const [error, setError] = useState<string | null>(null);

  const run = async (p: Promise<AgentInfo[]>, then?: () => void) => {
    try {
      setAgents(await p);
      setError(null);
      then?.();
    } catch (e) {
      setError(String(e));
    }
  };

  const launch = (a: AgentInfo) => {
    close();
    useStore.getState().launchAgent(a.id);
  };

  /** Ask the backend whether this agent may start. If it needs approval, show the review it built. */
  const start = async (a: AgentInfo) => {
    if (!workspace) return;
    try {
      const r = await agentApi.authorize(workspace.id, a.id);
      setError(null);
      if (r.kind === "allowed") launch(a);
      else setMode({ kind: "review", id: a.id, request: r.request });
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-overlay pt-20" onMouseDown={close}>
      <div className="max-h-[80vh] w-[38rem] overflow-auto rounded-lg border border-line bg-dialog p-4 shadow-xl" role="dialog" aria-modal="true" aria-label="Launch AI agent" onMouseDown={(e) => e.stopPropagation()} onKeyDown={(e) => e.key === "Escape" && close()}>
        {mode.kind === "list" && (
          <>
            <h2 className="mb-1 text-sm font-semibold">Launch an AI agent</h2>
            <p className="mb-3 text-xs text-muted">Runs in a terminal in <span className="text-cloud">{workspace?.name}</span>. Naurix starts the program only; it does not read or change the agent&apos;s own settings.</p>
            <ul className="space-y-1">
              {agents.map((a) => (
                <li key={a.id} className="flex items-center gap-2 rounded px-2 py-1.5 hover:bg-raised">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-sm">
                      <span>{a.name}</span>
                      {a.kind === "custom" && <span className="rounded bg-raised px-1 text-[10px] text-muted">custom</span>}
                      {a.adapter?.notice && <span className="text-warning" title={a.adapter.notice}>⚠</span>}
                    </div>
                    <div className="truncate text-[11px] text-faint" title={a.resolvedPath ?? undefined}>
                      {a.resolvedPath ? <span className="text-teal">✓ {a.resolvedPath}</span> : <span className="text-warning">Not found: {a.executable}</span>}
                    </div>
                  </div>
                  {a.kind === "custom" && (
                    <>
                      <button className="rounded px-2 py-0.5 text-xs text-muted hover:bg-line" onClick={() => setMode({ kind: "edit", draft: toDraft(a) })}>Edit</button>
                      <button className="rounded px-2 py-0.5 text-xs text-muted hover:bg-line" onClick={() => void run(agentApi.deleteCustom(a.id))}>Delete</button>
                    </>
                  )}
                  {a.resolvedPath ? (
                    // The backend decides: an agent with a matching approval starts, anything else is reviewed first.
                    <button className={`${btn} btn-primary`} onClick={() => void start(a)}>
                      Launch
                    </button>
                  ) : (
                    <button className={`${btn} bg-raised hover:bg-line`} onClick={() => setMode({ kind: "review", id: a.id })}>Set up…</button>
                  )}
                </li>
              ))}
            </ul>
            {error && <p className="mt-2 text-xs text-danger">{error}</p>}
            <div className="mt-3 flex justify-between">
              <button className={`${btn} bg-raised hover:bg-line`} onClick={() => setMode({ kind: "edit", draft: { name: "", executable: "", args: "", env: "" } })}>Add custom agent…</button>
              <button className={`${btn} bg-raised hover:bg-line`} onClick={close}>Close</button>
            </div>
          </>
        )}
        {mode.kind === "review" && <Review agent={agents.find((a) => a.id === mode.id)} request={mode.request} rootPath={workspace?.rootPath ?? ""} back={() => setMode({ kind: "list" })} start={start} launch={launch} run={run} error={error} />}
        {mode.kind === "edit" && <Edit draft={mode.draft} back={() => setMode({ kind: "list" })} run={run} error={error} />}
      </div>
    </div>
  );
}

function Review({ agent, request, rootPath, back, start, launch, run, error }: { agent?: AgentInfo; request?: ApprovalRequest; rootPath: string; back: () => void; start: (a: AgentInfo) => Promise<void>; launch: (a: AgentInfo) => void; run: (p: Promise<AgentInfo[]>, then?: () => void) => Promise<void>; error: string | null }) {
  const [path, setPath] = useState(agent?.pathOverride ?? "");
  const [version, setVersion] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const id = agent?.id;
  const approved = agent?.approved;
  useEffect(() => {
    // Only approved executables are asked for their version (it runs `--version`).
    if (id && approved) void agentApi.version(id).then(setVersion).catch(() => setVersion(null));
  }, [id, approved]);
  if (!agent) return null;

  /** Record the user's decision, then launch. The decision names only the request and a scope. */
  const allow = async (req: ApprovalRequest, scope: ApprovalScope) => {
    try {
      await approvalApi.decide(req.id, { decision: "allow", scope });
      launch(agent);
    } catch (e) {
      setProblem(String(e));
    }
  };
  const deny = async (req: ApprovalRequest) => {
    await approvalApi.decide(req.id, { decision: "deny" }).catch(() => {});
    back();
  };

  return (
    <>
      <h2 className="mb-1 text-sm font-semibold">{!agent.resolvedPath ? `Set up ${agent.name}` : request ? `Allow ${agent.name} to start?` : `Review: ${agent.name}`}</h2>
      {agent.adapter?.notice && <p className="mb-3 rounded border border-warning/40 bg-warning/10 p-2 text-xs text-warning">{agent.adapter.notice}</p>}
      {version && <p className="mb-2 text-xs text-faint">Installed version: <span className="font-mono text-muted">{version}</span></p>}
      <Capabilities agent={agent} />
      {agent.resolvedPath ? (
        <>
          <p className="mb-2 text-xs text-muted">{request ? "Naurix will run exactly the following in your workspace folder. Nothing else is executed." : "This is what would run in your workspace folder."}</p>
          <pre className="mb-2 overflow-auto rounded bg-midnight p-2 text-xs text-cloud" data-approval-display>{request ? request.display : agent.command}</pre>
          <dl className="mb-3 space-y-1 text-xs text-muted">
            <div><dt className="inline text-faint">Working directory: </dt><dd className="inline break-all">{rootPath}</dd></div>
          </dl>
        </>
      ) : (
        <p className="mb-3 text-xs text-warning">“{agent.executable}” was not found on this Mac. Install it, or choose where it is. Naurix never installs software for you.</p>
      )}
      <label className="mb-1 block text-xs text-faint">Executable path (optional)</label>
      <div className="mb-3 flex gap-2">
        <input className={field} value={path} placeholder={`Use “${agent.executable}” from PATH`} onChange={(e) => setPath(e.target.value)} />
        <button className={`${btn} bg-raised hover:bg-line`} onClick={() => void agentApi.pickExecutable().then((p) => p && setPath(p))}>Browse…</button>
        <button className={`${btn} bg-raised hover:bg-line`} onClick={() => void run(agentApi.setPath(agent.id, path.trim() || null))}>Save</button>
      </div>
      {(error || problem) && <p className="mb-2 text-xs text-danger">{problem ?? error}</p>}
      {request ? (
        <>
          <p className="mb-2 text-xs text-faint">A choice other than “once” is remembered for this exact program and arguments. If either changes you are asked again. You can withdraw approvals in Settings, Security.</p>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <button className={`${btn} bg-raised hover:bg-line`} onClick={() => void deny(request)}>Don’t allow</button>
            <div className="flex flex-wrap gap-2">
              {request.allowedScopes.map((sc, i) => (
                <button key={sc} className={`${btn} ${i === 0 ? "btn-primary" : "bg-raised hover:bg-line"}`} onClick={() => void allow(request, sc)}>{scopeLabel(sc)}</button>
              ))}
            </div>
          </div>
        </>
      ) : (
        <div className="flex justify-between">
          <button className={`${btn} bg-raised hover:bg-line`} onClick={back}>Back</button>
          <button disabled={!agent.resolvedPath} className={`${btn} btn-primary disabled:opacity-40`} onClick={() => void start(agent)}>Launch</button>
        </div>
      )}
    </>
  );
}

function Edit({ draft: initial, back, run, error }: { draft: Draft; back: () => void; run: (p: Promise<AgentInfo[]>, then?: () => void) => Promise<void>; error: string | null }) {
  const [d, setD] = useState(initial);
  const input: CustomAgentInput = {
    id: d.id,
    name: d.name,
    executable: d.executable.trim(),
    args: d.args.split("\n").map((l) => l.trimEnd()).filter((l) => l !== ""),
    env: d.env.split("\n").map((l) => l.trim()).filter(Boolean).map((l) => {
      const i = l.indexOf("=");
      return i < 0 ? { name: l, value: "" } : { name: l.slice(0, i).trim(), value: l.slice(i + 1) };
    }),
  };
  return (
    <>
      <h2 className="mb-3 text-sm font-semibold">{d.id ? "Edit custom agent" : "Add custom agent"}</h2>
      <label className="mb-1 block text-xs text-faint">Name</label>
      <input className={`${field} mb-2`} value={d.name} maxLength={60} onChange={(e) => setD({ ...d, name: e.target.value })} />
      <label className="mb-1 block text-xs text-faint">Executable (command name or absolute path)</label>
      <input className={`${field} mb-2`} value={d.executable} placeholder="aider" onChange={(e) => setD({ ...d, executable: e.target.value })} />
      <label className="mb-1 block text-xs text-faint">Arguments (one per line, no shell quoting needed)</label>
      <textarea className={`${field} mb-2 h-20 font-mono`} value={d.args} onChange={(e) => setD({ ...d, args: e.target.value })} />
      <label className="mb-1 block text-xs text-faint">Environment (NAME=value per line). Naurix stores no credentials: tokens, keys and passwords are refused. Set those in your shell profile.</label>
      <textarea className={`${field} mb-2 h-16 font-mono`} value={d.env} onChange={(e) => setD({ ...d, env: e.target.value })} />
      {error && <p className="mb-2 text-xs text-danger">{error}</p>}
      <div className="flex justify-between">
        <button className={`${btn} bg-raised hover:bg-line`} onClick={back}>Cancel</button>
        <button className={`${btn} btn-primary`} onClick={() => void run(agentApi.saveCustom(input), back)}>Save</button>
      </div>
    </>
  );
}

function toDraft(a: AgentInfo): Draft {
  return { id: a.id, name: a.name, executable: a.executable, args: a.args.join("\n"), env: a.env.map((e) => `${e.name}=${e.value}`).join("\n") };
}
