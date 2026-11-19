import { invoke } from "@tauri-apps/api/core";

// The core crate serializes these with snake_case keys.
export type FileEntry = {
  path: string;
  orig_path: string | null;
  kind: "changed" | "renamed" | "unmerged" | "untracked";
  index: string | null;
  worktree: string | null;
  submodule: boolean;
};

export type RepoStatus = {
  root: string;
  branch: string | null;
  detached: boolean;
  head_oid: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  stash: number;
  operation: "merging" | "rebasing" | "cherry-picking" | "reverting" | "bisecting" | null;
  counts: { staged: number; unstaged: number; untracked: number; conflicted: number };
  entries: FileEntry[];
  truncated: boolean;
};

export type StatusResult = { state: "not_repo" } | { state: "blocked"; root: string; keys: string[] } | ({ state: "repo" } & RepoStatus);
export type Area = "staged" | "unstaged" | "untracked";
export type DiffResult = { text: string; truncated: boolean; binary: boolean };
export type Worktree = { path: string; branch: string | null; head: string | null; detached: boolean; bare: boolean; locked: boolean };

export const gitApi = {
  status: (workspaceId: string) => invoke<StatusResult>("git_status", { workspaceId }),
  diff: (workspaceId: string, path: string, origPath: string | null, area: Area) => invoke<DiffResult>("git_diff", { workspaceId, path, origPath, area }),
  worktrees: (workspaceId: string) => invoke<Worktree[]>("git_worktrees", { workspaceId }),
  trust: (workspaceId: string) => invoke<StatusResult>("git_trust", { workspaceId }),
};

/** Entries grouped the way the panel shows them. A file with both staged and unstaged changes appears in both. */
export function groupEntries(entries: FileEntry[]) {
  return {
    conflicts: entries.filter((e) => e.kind === "unmerged"),
    staged: entries.filter((e) => e.kind !== "unmerged" && e.kind !== "untracked" && e.index !== null),
    unstaged: entries.filter((e) => e.kind !== "unmerged" && e.kind !== "untracked" && e.worktree !== null),
    untracked: entries.filter((e) => e.kind === "untracked"),
  };
}

export function branchLabel(s: Pick<RepoStatus, "branch" | "detached" | "head_oid">): string {
  return s.detached ? `HEAD detached${s.head_oid ? ` at ${s.head_oid.slice(0, 7)}` : ""}` : (s.branch ?? "(no commits yet)");
}

export function dirtyCount(s: Pick<RepoStatus, "counts">): number {
  const c = s.counts;
  return c.conflicted + c.staged + c.unstaged + c.untracked;
}
