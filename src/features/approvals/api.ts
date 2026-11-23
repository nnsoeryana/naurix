import { invoke } from "@tauri-apps/api/core";

/** How long an approval lasts (ADR 0027). "app" exists only for presets, for the exact executable path. */
export type ApprovalScope = "once" | "workspace" | "app";

/** What the backend asks the user to approve. Built by the backend; the UI shows `display` and answers by id. */
export type ApprovalRequest = {
  id: string;
  class: string;
  workspaceId: string;
  summary: string;
  /** The full command to review. Never persisted. */
  display: string;
  allowedScopes: ApprovalScope[];
  expiresInSecs: number;
};

export type ApprovalRecord = {
  id: string;
  class: string;
  scope: string;
  summary: string;
  workspaceId: string | null;
  workspaceName: string | null;
  /** active | consumed | revoked | expired */
  state: string;
  createdAt: string;
  lastUsedAt: string | null;
  endedAt: string | null;
};

export type Approvals = { active: ApprovalRecord[]; history: ApprovalRecord[] };

export const approvalApi = {
  decide: (requestId: string, decision: { decision: "deny" } | { decision: "allow"; scope: ApprovalScope }) =>
    invoke<void>("approval_decide", { requestId, decision }),
  list: () => invoke<Approvals>("approval_list"),
  revoke: (id: string) => invoke<boolean>("approval_revoke", { id }),
  revokeAll: () => invoke<number>("approval_revoke_all"),
};

const LABELS: Record<ApprovalScope, string> = {
  once: "Allow once",
  workspace: "Allow in this workspace",
  app: "Always allow this program",
};

/** Button text for a scope the backend offered. */
export const scopeLabel = (s: ApprovalScope): string => LABELS[s];

/** Short description of a stored grant's scope, for the Settings list. */
export function describeScope(scope: string, workspaceName: string | null): string {
  if (scope === "app") return "Every workspace";
  if (scope === "workspace") return workspaceName ? `Workspace: ${workspaceName}` : "One workspace";
  if (scope === "once") return "Once";
  return scope;
}
