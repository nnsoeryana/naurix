import { useEffect, useState } from "react";
import { useStore } from "../../state/store";
import { approvalApi, scopeLabel, type ApprovalRequest, type ApprovalScope } from "../approvals/api";
import { worktreeApi, type WorktreeSupport } from "../worktrees/api";
import { agentApi, parseExitCodes, type AgentInfo, type CustomAgentInput, type Role } from "./api";
import { extrasAreOnceOnly, ordered, requestTitle, splitRequests } from "./requests";
import { Capabilities } from "./Capabilities";

/** `requests` is set when the backend asked for approval: the review then shows ITS text, not anything the UI built. */
type Mode = { kind: "list" } | { kind: "review"; id: string; requests?: ApprovalRequest[] } | { kind: "edit"; draft: Draft };
type Draft = { id?: string; name: string; executable: string; args: string; env: string; role: Role | null; exitCodes: string };

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
  // Opt-in worktree (ADR 0028): off by default, offered only where the backend says it can work.
  const [wt, setWt] = useState(false);
  const [support, setSupport] = useState<WorktreeSupport | null>(null);
  const [sameFolder, setSameFolder] = useState(0);
  const wsId = workspace?.id;
  useEffect(() => {
    if (!wsId) return;
    worktreeApi.support(wsId).then(setSupport).catch(() => setSupport(null));
    worktreeApi.agentsInFolder(wsId).then(setSameFolder).catch(() => setSameFolder(0));
  }, [wsId]);

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
    useStore.getState().launchAgent(a.id, { worktree: wt });
  };

  /** Ask the backend whether this agent may start. If it needs approval, show the review it built. */
  const start = async (a: AgentInfo) => {
    if (!workspace) return;
    try {
      const r = await agentApi.authorize(workspace.id, a.id, wt);
      setError(null);
      if (r.kind === "allowed") launch(a);
      else setMode({ kind: "review", id: a.id, requests: r.requests });
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
                      {a.role && <span className="rounded bg-raised px-1 text-[10px] text-muted">{a.role} team</span>}
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
            <div className="mt-3 rounded border border-line/60 p-2" data-worktree-option>
              <label className={`flex items-start gap-2 text-xs ${support?.state === "ok" ? "text-cloud" : "text-faint"}`}>
                <input type="checkbox" className="mt-0.5" checked={wt && support?.state === "ok"} disabled={support?.state !== "ok"} onChange={(e) => setWt(e.target.checked)} aria-label="Run in a separate worktree" />
                <span>
                  Run in a separate worktree
                  <span className="block text-[11px] text-faint">{support?.state === "unavailable" ? support.reason : "Gives this agent its own branch and folder so it cannot overwrite your files or another agent’s. Creating it needs your approval; the branch is kept when it is removed."}</span>
                </span>
              </label>
              {sameFolder > 0 && !wt && support?.state === "ok" && (
                <p className="mt-2 text-[11px] text-warning" data-same-folder-warning>
                  {sameFolder === 1 ? "Another agent is already running in this folder." : `${sameFolder} agents are already running in this folder.`} Agents in the same folder can overwrite each other’s files. A separate worktree avoids that.
                </p>
              )}
            </div>
            {error && <p className="mt-2 text-xs text-danger">{error}</p>}
            <div className="mt-3 flex justify-between">
              <button className={`${btn} bg-raised hover:bg-line`} onClick={() => setMode({ kind: "edit", draft: { name: "", executable: "", args: "", env: "", role: null, exitCodes: "0" } })}>Add custom agent…</button>
              <button className={`${btn} bg-raised hover:bg-line`} onClick={close}>Close</button>
            </div>
          </>
        )}
        {mode.kind === "review" && <Review agent={agents.find((a) => a.id === mode.id)} requests={mode.requests} rootPath={workspace?.rootPath ?? ""} back={() => setMode({ kind: "list" })} start={start} launch={launch} run={run} error={error} />}
        {mode.kind === "edit" && <Edit draft={mode.draft} back={() => setMode({ kind: "list" })} run={run} error={error} />}
      </div>
    </div>
  );
}

function Review({ agent, requests, rootPath, back, start, launch, run, error }: { agent?: AgentInfo; requests?: ApprovalRequest[]; rootPath: string; back: () => void; start: (a: AgentInfo) => Promise<void>; launch: (a: AgentInfo) => void; run: (p: Promise<AgentInfo[]>, then?: () => void) => Promise<void>; error: string | null }) {
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
  const split = requests ? splitRequests(requests) : null;
  // The scope choices belong to the main request. Extra requests that can only ever be approved once
  // (creating a worktree) are approved once alongside it; anything else is never decided for the user.
  const canProceed = !!split && extrasAreOnceOnly(split.also);

  /** Record the user's decisions, then launch. Each decision names only a request and a scope. */
  const allow = async (scope: ApprovalScope) => {
    if (!split) return;
    try {
      for (const extra of split.also) await approvalApi.decide(extra.id, { decision: "allow", scope: "once" });
      await approvalApi.decide(split.main.id, { decision: "allow", scope });
      launch(agent);
    } catch (e) {
      setProblem(String(e));
    }
  };
  const deny = async () => {
    for (const r of requests ?? []) await approvalApi.decide(r.id, { decision: "deny" }).catch(() => {});
    back();
  };

  return (
    <>
      <h2 className="mb-1 text-sm font-semibold">{!agent.resolvedPath ? `Set up ${agent.name}` : split ? `Allow ${agent.name} to start?` : `Review: ${agent.name}`}</h2>
      {agent.adapter?.notice && <p className="mb-3 rounded border border-warning/40 bg-warning/10 p-2 text-xs text-warning">{agent.adapter.notice}</p>}
      {version && <p className="mb-2 text-xs text-faint">Installed version: <span className="font-mono text-muted">{version}</span></p>}
      <Capabilities agent={agent} />
      {agent.resolvedPath ? (
        <>
          <p className="mb-2 text-xs text-muted">{split ? "Naurix will do exactly the following, in this order. Nothing else is executed." : "This is what would run in your workspace folder."}</p>
          {split ? (
            ordered(requests!).map((r, i, all) => (
              <div key={r.id} className="mb-2">
                {all.length > 1 && <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-faint">{i + 1}. {requestTitle(r)}</div>}
                <pre className="overflow-auto whitespace-pre-wrap rounded bg-midnight p-2 text-xs text-cloud" data-approval-display>{r.display}</pre>
              </div>
            ))
          ) : (
            <pre className="mb-2 overflow-auto rounded bg-midnight p-2 text-xs text-cloud" data-approval-display>{agent.command}</pre>
          )}
          <dl className="mb-3 space-y-1 text-xs text-muted">
            <div><dt className="inline text-faint">Working directory: </dt><dd className="inline break-all">{split && split.also.length > 0 ? "the new worktree" : rootPath}</dd></div>
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
      {split ? (
        <>
          <p className="mb-2 text-xs text-faint">A choice other than “once” is remembered for this exact program and arguments. If either changes you are asked again. {split.also.length > 0 ? "Creating the worktree is a change to your Git repository, so it is approved once, every time. " : ""}You can withdraw approvals in Settings, Security.</p>
          {!canProceed && <p className="mb-2 text-xs text-danger">This needs a separate decision that this dialog does not make for you.</p>}
          <div className="flex flex-wrap items-center justify-between gap-2">
            <button className={`${btn} bg-raised hover:bg-line`} onClick={() => void deny()}>Don’t allow</button>
            <div className="flex flex-wrap gap-2">
              {split.main.allowedScopes.map((sc, i) => (
                <button key={sc} disabled={!canProceed} className={`${btn} ${i === 0 ? "btn-primary" : "bg-raised hover:bg-line"} disabled:opacity-40`} onClick={() => void allow(sc)}>{scopeLabel(sc)}</button>
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
  const codes = parseExitCodes(d.exitCodes);
  const input: CustomAgentInput = {
    id: d.id,
    name: d.name,
    executable: d.executable.trim(),
    args: d.args.split("\n").map((l) => l.trimEnd()).filter((l) => l !== ""),
    env: d.env.split("\n").map((l) => l.trim()).filter(Boolean).map((l) => {
      const i = l.indexOf("=");
      return i < 0 ? { name: l, value: "" } : { name: l.slice(0, i).trim(), value: l.slice(i + 1) };
    }),
    role: d.role,
    successExitCodes: codes ?? [],
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
      <div className="mb-2 flex gap-3">
        <div className="flex-1">
          <label className="mb-1 block text-xs text-faint">Security role (a label only)</label>
          <select className={field} value={d.role ?? ""} onChange={(e) => setD({ ...d, role: (e.target.value || null) as Role | null })}>
            <option value="">None</option>
            <option value="red">Red team</option>
            <option value="blue">Blue team</option>
            <option value="purple">Purple team</option>
          </select>
        </div>
        <div className="flex-1">
          <label className="mb-1 block text-xs text-faint">Success exit codes in workflows</label>
          <input className={`${field} font-mono`} value={d.exitCodes} placeholder="0" onChange={(e) => setD({ ...d, exitCodes: e.target.value })} />
        </div>
      </div>
      <p className="mb-2 text-xs text-faint">Scanners often exit with 1 when they find issues. Listing it lets the next workflow step start. Changing the codes asks for approval again.</p>
      {codes === null && <p className="mb-2 text-xs text-danger">Exit codes are whole numbers from 0 to 255, separated by commas.</p>}
      {error && <p className="mb-2 text-xs text-danger">{error}</p>}
      <div className="flex justify-between">
        <button className={`${btn} bg-raised hover:bg-line`} onClick={back}>Cancel</button>
        <button className={`${btn} btn-primary`} disabled={codes === null} onClick={() => void run(agentApi.saveCustom(input), back)}>Save</button>
      </div>
    </>
  );
}

function toDraft(a: AgentInfo): Draft {
  return {
    id: a.id,
    name: a.name,
    executable: a.executable,
    args: a.args.join("\n"),
    env: a.env.map((e) => `${e.name}=${e.value}`).join("\n"),
    role: a.role,
    exitCodes: a.successExitCodes.join(", "),
  };
}
