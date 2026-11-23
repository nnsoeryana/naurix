import { useCallback, useEffect, useState } from "react";
import { worktreeApi, type ManagedWorktree } from "./api";
import { forgetMissing, startRemove } from "./remove";

const outline = "rounded border border-line px-2 py-0.5 text-xs text-muted hover:bg-raised disabled:opacity-40";

/** Settings, Security: worktrees whose workspace was removed. Their folders were left alone, so they can be cleaned up here. */
export function OrphanWorktrees() {
  const [items, setItems] = useState<ManagedWorktree[] | null>(null);
  const load = useCallback(() => worktreeApi.list().then((all) => setItems(all.filter((w) => w.workspaceId === null))).catch(() => setItems([])), []);
  useEffect(() => void load(), [load]);
  if (!items) return <span className="text-xs text-faint">Loading…</span>;
  if (items.length === 0) return <span className="text-xs text-faint">None.</span>;
  return (
    <ul className="space-y-1" data-orphan-worktrees>
      {items.map((w) => (
        <li key={w.id} className="flex items-center gap-3">
          <span className="min-w-0 flex-1">
            <span className="block truncate font-mono text-xs text-cloud" title={w.path}>{w.branch}</span>
            <span className="block truncate text-[11px] text-faint" title={w.path}>{w.path}</span>
          </span>
          {w.exists ? <button className={outline} disabled={w.inUse} title={w.inUse ? "Something is still running in it" : undefined} onClick={() => startRemove(w, load)}>Remove…</button> : <button className={outline} onClick={() => forgetMissing(w, load)} title="The folder is already gone">Forget</button>}
        </li>
      ))}
    </ul>
  );
}
