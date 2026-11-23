import { invoke } from "@tauri-apps/api/core";
import type { ApprovalRequest } from "../approvals/api";

/** A worktree Naurix created for an agent (ADR 0028). The UI names it by id; it never supplies a path. */
export type ManagedWorktree = {
  id: string;
  /** Null once the workspace it was created for has been removed. */
  workspaceId: string | null;
  repoRoot: string;
  path: string;
  branch: string;
  baseCommit: string;
  createdAt: string;
  /** The folder is still there. */
  exists: boolean;
  /** A session is running in it right now. */
  inUse: boolean;
};

export type WorktreeSupport = { state: "ok" } | { state: "unavailable"; reason: string };

export const worktreeApi = {
  /** One workspace's worktrees, or all of them when no workspace is given. */
  list: (workspaceId?: string) => invoke<ManagedWorktree[]>("worktree_list", { workspaceId: workspaceId ?? null }),
  /** Can "run in a separate worktree" be offered here? Changes nothing. */
  support: (workspaceId: string) => invoke<WorktreeSupport>("worktree_support", { workspaceId }),
  /** Agents already running in the workspace's own folder (a reason to suggest a worktree). */
  agentsInFolder: (workspaceId: string) => invoke<number>("agents_in_workspace_folder", { workspaceId }),
  forSession: (id: number) => invoke<ManagedWorktree | null>("session_worktree", { id }),
  /** The review for removing one, built by the backend. Rejects with the reason when it cannot be removed. */
  reviewRemove: (id: string) => invoke<ApprovalRequest>("worktree_review_remove", { id }),
  remove: (id: string) => invoke<void>("worktree_remove", { id }),
  /** The folder is already gone: forget Naurix's record of it. */
  forget: (id: string) => invoke<void>("worktree_forget", { id }),
};
