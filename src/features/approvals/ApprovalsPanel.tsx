import { useCallback, useEffect, useState } from "react";
import { approvalApi, describeScope, type Approvals } from "./api";

const outline = "rounded border border-line px-2 py-0.5 text-xs text-muted hover:bg-raised disabled:opacity-40";

/** Settings, Security: what the user has allowed agents to do, with a way to take it back (ADR 0027). */
export function ApprovalsPanel() {
  const [data, setData] = useState<Approvals | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => approvalApi.list().then((d) => (setData(d), setError(null))).catch((e) => setError(String(e))), []);
  useEffect(() => void load(), [load]);

  if (!data) return <span className="text-xs text-faint">{error ?? "Loading…"}</span>;
  const act = (p: Promise<unknown>) => void p.then(load).catch((e) => setError(String(e)));
  return (
    <div className="w-full space-y-2" data-approvals>
      {data.active.length === 0 ? (
        <span className="text-xs text-faint">None. You are asked before an agent starts.</span>
      ) : (
        <ul className="space-y-1">
          {data.active.map((a) => (
            <li key={a.id} className="flex items-center gap-3" data-approval={a.scope}>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-xs text-cloud" title={a.summary}>{a.summary}</span>
                <span className="block text-[11px] text-faint">{describeScope(a.scope, a.workspaceName)}{a.lastUsedAt ? ` · last used ${a.lastUsedAt}` : " · not used yet"}</span>
              </span>
              <button className={outline} onClick={() => act(approvalApi.revoke(a.id))}>Revoke</button>
            </li>
          ))}
        </ul>
      )}
      {data.active.length > 1 && <button className={outline} onClick={() => act(approvalApi.revokeAll())}>Revoke all</button>}
      {data.history.length > 0 && (
        <details className="text-[11px] text-faint">
          <summary className="cursor-pointer">Recent: {data.history.length} used, withdrawn or expired</summary>
          <ul className="mt-1 space-y-0.5">{data.history.slice(0, 10).map((h) => <li key={h.id} className="truncate" title={h.summary}>{h.state} · {describeScope(h.scope, h.workspaceName)} · {h.summary}</li>)}</ul>
        </details>
      )}
      {error && <p className="text-xs text-danger">{error}</p>}
    </div>
  );
}
