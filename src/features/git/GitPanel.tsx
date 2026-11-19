import { useEffect, useState } from "react";
import { useStore } from "../../state/store";
import { tildePath } from "../workspaces/path";
import { branchLabel, gitApi, groupEntries, type Area, type DiffResult, type FileEntry, type RepoStatus, type Worktree } from "./api";

const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";
const badge: Record<string, string> = { M: "text-warning", A: "text-teal", D: "text-danger", R: "text-purple", C: "text-purple", T: "text-warning", U: "text-danger", "?": "text-faint" };
const opText: Record<string, string> = { merging: "Merge in progress", rebasing: "Rebase in progress", "cherry-picking": "Cherry-pick in progress", reverting: "Revert in progress", bisecting: "Bisect in progress" };

type Sel = { path: string; orig: string | null; area: Area };

function lineClass(l: string): string {
  if (l.startsWith("+++") || l.startsWith("---") || l.startsWith("diff ") || l.startsWith("index ") || l.startsWith("new file") || l.startsWith("deleted file") || l.startsWith("rename ") || l.startsWith("similarity")) return "text-faint";
  if (l.startsWith("@@")) return "text-purple";
  if (l.startsWith("+")) return "bg-teal/10 text-teal";
  if (l.startsWith("-")) return "bg-danger/10 text-danger";
  return "text-cloud/80";
}

/** Read-only Git view (M9): branch, state, changed files and a bounded diff. Never changes the repository. */
export function GitPanel() {
  const git = useStore((s) => s.git);
  const workspaceId = useStore((s) => s.activeWorkspaceId);
  const refresh = useStore((s) => s.refreshGit);
  const trustRepo = useStore((s) => s.trustRepo);
  const newRepoTab = useStore((s) => s.newRepoTab);
  const [sel, setSel] = useState<Sel | null>(null);
  const [diff, setDiff] = useState<DiffResult | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [trees, setTrees] = useState<Worktree[]>([]);
  const r = git.result?.state === "repo" ? (git.result as { state: "repo" } & RepoStatus) : null;

  // Reload the selected file's diff whenever the status is refreshed (or the selection changes).
  useEffect(() => {
    if (!sel || !workspaceId) return setDiff(null);
    let live = true;
    setDiffError(null);
    gitApi.diff(workspaceId, sel.path, sel.orig, sel.area).then((d) => live && setDiff(d)).catch((e) => live && (setDiff(null), setDiffError(String(e))));
    return () => {
      live = false;
    };
  }, [sel, workspaceId, git.result]);

  useEffect(() => {
    setSel(null);
  }, [workspaceId]);

  useEffect(() => {
    if (!workspaceId || !r) return setTrees([]);
    gitApi.worktrees(workspaceId).then(setTrees).catch(() => setTrees([]));
  }, [workspaceId, r?.root, r?.head_oid]); // eslint-disable-line react-hooks/exhaustive-deps

  const groups = r ? groupEntries(r.entries) : null;

  const row = (e: FileEntry, area: Area, letter: string | null) => {
    const selected = sel?.path === e.path && sel.area === area;
    return (
      <li key={`${area}:${e.path}`}>
        <button className={`flex w-full items-center gap-2 rounded px-2 py-0.5 text-left font-mono text-xs ${selected ? "bg-purple/20" : "hover:bg-raised/60"}`} onClick={() => setSel({ path: e.path, orig: e.orig_path, area })} title={e.path}>
          <span className={`w-3 shrink-0 text-center font-semibold ${badge[letter ?? "?"] ?? "text-muted"}`}>{letter ?? "?"}</span>
          <span className="truncate">{e.orig_path ? `${e.orig_path} → ${e.path}` : e.path}</span>
          {e.submodule && <span className="ml-auto shrink-0 text-[10px] text-faint">submodule</span>}
        </button>
      </li>
    );
  };

  const section = (title: string, items: FileEntry[], area: Area, pick: (e: FileEntry) => string | null) =>
    items.length > 0 && (
      <section className="mb-4" data-git-section={title}>
        <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-faint">{title} <span className="text-muted">({items.length})</span></h3>
        <ul>{items.map((e) => row(e, area, pick(e)))}</ul>
      </section>
    );

  return (
    <div className="absolute inset-0 flex flex-col bg-midnight">
      <div className="shrink-0 border-b border-line px-8 py-5">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h1 className="text-2xl font-semibold">Git</h1>
            {r ? <p className="mt-1 truncate font-mono text-xs text-faint" title={r.root}>{tildePath(r.root)}</p> : <p className="mt-1 text-sm text-muted">Read-only view of the active workspace&apos;s repository.</p>}
          </div>
          <div className="flex shrink-0 gap-2">
            <button className={outline} disabled={!r} onClick={() => r && newRepoTab(tildePath(r.root))} title="Open a terminal at the repository root">Open in terminal</button>
            <button className={outline} disabled={git.loading || !workspaceId} onClick={() => void refresh()}>{git.loading ? "Refreshing…" : "Refresh"}</button>
          </div>
        </div>
        {r && (
          <div className="mt-3 flex flex-wrap items-center gap-2 text-xs" data-git-header>
            <span className="rounded-full border border-purple/50 px-3 py-0.5 font-mono text-purple">⎇ {branchLabel(r)}</span>
            {r.upstream && <span className="rounded-full border border-line px-2 py-0.5 text-muted" title={`Upstream ${r.upstream}`}>{r.ahead > 0 || r.behind > 0 ? `↑${r.ahead} ↓${r.behind}` : "up to date"} · {r.upstream}</span>}
            {!r.upstream && r.branch && <span className="text-faint">no upstream</span>}
            {r.stash > 0 && <span className="rounded-full border border-line px-2 py-0.5 text-muted">{r.stash} stash{r.stash > 1 ? "es" : ""}</span>}
            {r.operation && <span className="rounded-full border border-warning/50 px-2 py-0.5 text-warning">{opText[r.operation]}</span>}
            <span className="text-faint">{r.counts.conflicted > 0 && `${r.counts.conflicted} conflicts · `}{r.counts.staged} staged · {r.counts.unstaged} changed · {r.counts.untracked} untracked</span>
          </div>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {!workspaceId && <p className="p-8 text-sm text-muted">Open a project to see its Git status.</p>}
        {git.error && <p className="p-8 text-sm text-danger">{git.error}</p>}
        {git.result?.state === "not_repo" && (
          <div className="p-8 text-sm text-muted">
            <p className="text-cloud">This folder is not a Git repository.</p>
            <p className="mt-1">Run <code className="font-mono">git init</code> in a terminal here if you want version control. Naurix only shows Git status; it never changes your repository.</p>
          </div>
        )}
        {git.result?.state === "blocked" && (
          <div className="m-8 max-w-xl rounded-xl border border-warning/40 bg-warning/10 p-4 text-sm text-warning">
            <p className="font-medium">This repository runs commands from its own configuration.</p>
            <p className="mt-1 text-xs">Its local Git config defines: <span className="font-mono">{git.result.keys.join(", ")}</span>. Git would run those commands just to show status, so Naurix has not run Git here. Only continue if you trust this repository.</p>
            <button className="btn-primary mt-3 rounded px-3 py-1 text-xs" onClick={() => useStore.getState().setDialog({ kind: "confirm", title: "Trust this repository?", message: `Git will be allowed to run the commands defined in ${git.result?.state === "blocked" ? git.result.root : ""}'s own configuration whenever Naurix shows its status.`, confirmLabel: "Trust", danger: true, onConfirm: () => void trustRepo() })}>Trust this repository…</button>
          </div>
        )}
        {r && groups && (
          <div className="flex min-h-full">
            <div className="w-[22rem] shrink-0 overflow-auto border-r border-line p-4">
              {r.entries.length === 0 && <p className="text-sm text-muted">Working tree clean. Nothing to commit.</p>}
              {section("Conflicts", groups.conflicts, "unstaged", () => "U")}
              {section("Staged", groups.staged, "staged", (e) => e.index)}
              {section("Changes", groups.unstaged, "unstaged", (e) => e.worktree)}
              {section("Untracked", groups.untracked, "untracked", () => "?")}
              {r.truncated && <p className="text-[11px] text-warning">Only the first files are listed. The repository has more changes than Naurix shows.</p>}
              {trees.length > 1 && (
                <section className="mt-6" data-git-section="Worktrees">
                  <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-faint">Worktrees ({trees.length})</h3>
                  <ul className="space-y-1 text-xs">{trees.map((t) => <li key={t.path} className="truncate font-mono text-muted" title={t.path}>{t.bare ? "(bare)" : t.branch ?? (t.detached ? "detached" : "?")} <span className="text-faint">{tildePath(t.path)}</span></li>)}</ul>
                </section>
              )}
            </div>
            <div className="min-w-0 flex-1 overflow-auto p-4" data-git-diff>
              {!sel && <p className="text-sm text-faint">Select a file to see its changes.</p>}
              {diffError && <p className="text-sm text-danger">{diffError}</p>}
              {sel && diff && (
                <>
                  <p className="mb-2 font-mono text-xs text-muted">{sel.path} <span className="text-faint">({sel.area})</span></p>
                  {diff.binary ? <p className="text-sm text-muted">Binary file: no text diff.</p> : diff.text.trim() === "" ? <p className="text-sm text-muted">No textual changes.</p> : (
                    <pre className="overflow-auto rounded-lg border border-line bg-surface/60 p-3 font-mono text-[12px] leading-5">{diff.text.split("\n").map((l, i) => <div key={i} className={lineClass(l)}>{l || " "}</div>)}</pre>
                  )}
                  {diff.truncated && <p className="mt-2 text-xs text-warning">The diff is longer than Naurix shows and was cut off.</p>}
                </>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
