import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../state/store";
import { mcpApi, summarize, type CatalogEntry, type McpInput, type McpServer, type ProbeResult, type SyncAgent, type SyncPlan, type SyncRun } from "./api";
import { ApprovalDialog, CatalogDialog, EditorDialog } from "./Dialogs";
import { SyncPanel } from "./SyncPanel";

const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";

function statusChip(s: McpServer): { text: string; tone: string } {
  if (!s.approved) return { text: "Needs approval", tone: "border-warning/50 text-warning" };
  if (s.lastStatus === "connected") return { text: `Connected · ${s.lastToolCount ?? 0} tools`, tone: "border-teal/50 text-teal" };
  if (s.lastStatus === "failed") return { text: "Test failed", tone: "border-danger/50 text-danger" };
  return { text: "Approved · not tested", tone: "border-line text-muted" };
}

/** MCP Integrations screen: servers, catalog, approval, tests, and sync to agents (M7). */
export function McpManager() {
  const workspaceId = useStore((s) => s.activeWorkspaceId);
  const workspace = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId));
  const setNotice = useStore((s) => s.setNotice);
  const [servers, setServers] = useState<McpServer[]>([]);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [agents, setAgents] = useState<SyncAgent[]>([]);
  const [probe, setProbe] = useState<Record<string, ProbeResult | "running">>({});
  const [dialog, setDialog] = useState<null | { kind: "catalog" } | { kind: "edit"; input: McpInput; isNew: boolean } | { kind: "approve"; id: string }>(null);
  const [secretFor, setSecretFor] = useState<{ id: string; name: string } | null>(null);
  const [secretValue, setSecretValue] = useState("");

  const fail = useCallback((e: unknown) => setNotice(String(e)), [setNotice]);
  const refresh = useCallback(() => mcpApi.list().then(setServers).catch(fail), [fail]);

  useEffect(() => {
    void refresh();
    mcpApi.catalog().then(setCatalog).catch(fail);
    mcpApi.syncAgents().then(setAgents).catch(fail);
  }, [refresh, fail]);

  const run = (p: Promise<McpServer[]>) => p.then(setServers).catch(fail);

  const test = (s: McpServer) => {
    setProbe((m) => ({ ...m, [s.id]: "running" }));
    mcpApi
      .test(s.id)
      .then((r) => {
        setProbe((m) => ({ ...m, [s.id]: r }));
        void refresh();
      })
      .catch((e) => {
        setProbe((m) => {
          const { [s.id]: _x, ...rest } = m;
          return rest;
        });
        fail(e);
      });
  };

  const approving = dialog?.kind === "approve" ? servers.find((s) => s.id === dialog.id) : undefined;

  return (
    <div className="absolute inset-0 overflow-auto bg-midnight">
      <div className="mx-auto max-w-3xl px-8 pb-16 pt-10">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold">MCP Integrations</h1>
            <p className="mt-1 max-w-xl text-sm text-muted">Tools your agents can use. Servers are defined once here, you approve each one, and Naurix can write them into the agent config files it has verified.</p>
          </div>
          <div className="flex shrink-0 gap-2">
            <button className={outline} onClick={() => setDialog({ kind: "catalog" })}>Add from catalog…</button>
            <button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => setDialog({ kind: "edit", isNew: true, input: { name: "", transport: "stdio", command: "", args: [], url: null, env: [], headers: [], secrets: [] } })}>Add manually…</button>
          </div>
        </div>

        <ul className="mt-6 space-y-3">
          {servers.length === 0 && <li className="rounded-lg border border-dashed border-line p-6 text-center text-sm text-faint">No MCP servers yet. Add one from the catalog, or define your own.</li>}
          {servers.map((s) => {
            const chip = statusChip(s);
            const r = probe[s.id];
            return (
              <li key={s.id} className="rounded-xl border border-line bg-surface/60 p-4" data-mcp-server={s.name}>
                <div className="flex items-start gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{s.name}</span>
                      <span className="rounded bg-raised px-1.5 text-[10px] uppercase tracking-wide text-muted">{s.transport === "stdio" ? "local" : "remote"}</span>
                      <span className={`rounded-full border px-2 text-[11px] ${chip.tone}`}>{chip.text}</span>
                    </div>
                    <p className="mt-1 truncate font-mono text-xs text-faint" title={summarize(s)}>{summarize(s)}</p>
                  </div>
                  <label className="flex shrink-0 items-center gap-2 text-xs text-muted" title="Disabled servers are not written to agent configs">
                    <input type="checkbox" checked={s.enabled} onChange={(e) => void run(mcpApi.setEnabled(s.id, e.target.checked))} /> Enabled
                  </label>
                </div>

                {s.secrets.length > 0 && (
                  <ul className="mt-3 space-y-1 border-t border-line pt-3 text-xs">
                    {s.secrets.map((sec) => (
                      <li key={sec.name} className="flex items-center gap-2">
                        <span className="font-mono text-muted">{sec.name}</span>
                        <span className={sec.hasValue ? "text-teal" : "text-warning"}>{sec.hasValue ? "● stored in system keyring" : "○ not set"}</span>
                        {secretFor?.id === s.id && secretFor.name === sec.name ? (
                          <form
                            className="ml-auto flex items-center gap-2"
                            onSubmit={(e) => {
                              e.preventDefault();
                              if (!secretValue) return;
                              void run(mcpApi.secretSet(s.id, sec.name, secretValue)).then(() => {
                                setSecretValue("");
                                setSecretFor(null);
                              });
                            }}
                          >
                            <input type="password" autoFocus autoComplete="off" value={secretValue} onChange={(e) => setSecretValue(e.target.value)} placeholder="Paste value" className="w-56 rounded border border-line bg-midnight px-2 py-0.5 text-xs outline-none focus:border-purple" />
                            <button type="submit" className="btn-primary rounded px-2 py-0.5">Save</button>
                            <button type="button" className={outline} onClick={() => (setSecretValue(""), setSecretFor(null))}>Cancel</button>
                          </form>
                        ) : (
                          <span className="ml-auto flex gap-2">
                            <button className={outline} onClick={() => setSecretFor({ id: s.id, name: sec.name })}>{sec.hasValue ? "Replace…" : "Set value…"}</button>
                            {sec.hasValue && <button className={outline} onClick={() => void run(mcpApi.secretClear(s.id, sec.name))}>Remove</button>}
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                )}

                <div className="mt-3 flex flex-wrap gap-2">
                  {!s.approved ? (
                    <button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => setDialog({ kind: "approve", id: s.id })}>Review &amp; approve…</button>
                  ) : (
                    <button className={outline} disabled={r === "running"} onClick={() => test(s)} title={s.transport === "http" ? "Remote servers are tested by the agent that connects to them" : "Starts the server and asks for its tools"}>
                      {r === "running" ? "Testing…" : "Test connection"}
                    </button>
                  )}
                  <button className={outline} onClick={() => setDialog({ kind: "edit", isNew: false, input: { id: s.id, name: s.name, transport: s.transport, command: s.command, args: s.args, url: s.url, env: s.env, headers: s.headers, secrets: s.secrets.map((x) => x.name) } })}>Edit…</button>
                  <button
                    className={outline}
                    onClick={() =>
                      useStore.getState().setDialog({ kind: "confirm", title: `Delete “${s.name}”?`, message: "Its stored secrets are removed from the system keyring. Entries Naurix wrote into agent configs are removed the next time you sync.", confirmLabel: "Delete", danger: true, onConfirm: () => void run(mcpApi.remove(s.id)) })
                    }
                  >
                    Delete
                  </button>
                </div>

                {s.lastError && !r && <p className="mt-2 text-xs text-danger">Last test: {s.lastError}</p>}
                {r && r !== "running" && (
                  <div className="mt-3 rounded-lg bg-midnight p-3 text-xs" data-mcp-result={s.name}>
                    {r.status === "connected" ? (
                      <>
                        <p className="text-teal">Connected to {r.server_name ?? "server"} {r.server_version ?? ""} · protocol {r.protocol} · {r.tools.length} tools</p>
                        <ul className="mt-2 max-h-40 space-y-1 overflow-auto">
                          {r.tools.map((t) => (
                            <li key={t.name} className="flex gap-2"><span className="font-mono text-cloud/90">{t.name}</span><span className="truncate text-faint">{t.description}</span></li>
                          ))}
                        </ul>
                      </>
                    ) : (
                      <p className={r.status === "failed" ? "text-danger" : "text-muted"}>{r.error}</p>
                    )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>

        <SyncPanel workspaceId={workspaceId} workspaceName={workspace?.name ?? null} agents={agents} onError={fail} />
      </div>

      {dialog?.kind === "catalog" && (
        <CatalogDialog
          entries={catalog}
          onClose={() => setDialog(null)}
          onPick={(e) =>
            void mcpApi
              .catalogInput(e.id, workspaceId)
              .then((input) => setDialog({ kind: "edit", input, isNew: true }))
              .catch(fail)
          }
        />
      )}
      {dialog?.kind === "edit" && (
        <EditorDialog
          initial={dialog.input}
          isNew={dialog.isNew}
          onClose={() => setDialog(null)}
          onSave={(input) =>
            mcpApi.save(input).then((list) => {
              setServers(list);
              setDialog(null);
            })
          }
        />
      )}
      {approving && (
        <ApprovalDialog
          server={approving}
          onClose={() => setDialog(null)}
          onApprove={() =>
            void mcpApi
              .approve(approving.id, approving.fingerprint)
              .then((l) => (setServers(l), setDialog(null)))
              .catch(fail)
          }
        />
      )}
    </div>
  );
}

export type { SyncPlan, SyncRun };
