import type { ApprovalRequest } from "../approvals/api";

/**
 * A launch can need more than one approval (ADR 0028): the agent itself, and a worktree for it. The
 * agent request is the one the user chooses a scope for. Any other request that can only ever be
 * approved once (creating a worktree is a Git change, once only) is approved once together with it,
 * so the user answers a single dialog. A request that offers more than "once" is never decided for the user.
 */
export function splitRequests(requests: ApprovalRequest[]): { main: ApprovalRequest; also: ApprovalRequest[] } | null {
  if (requests.length === 0) return null;
  const main = requests.find((r) => r.class === "launch_agent") ?? requests[0];
  const also = requests.filter((r) => r !== main);
  return { main, also };
}

/** True when every extra request can only be approved once, so approving it for the user is safe. */
export function extrasAreOnceOnly(also: ApprovalRequest[]): boolean {
  return also.every((r) => r.allowedScopes.length === 1 && r.allowedScopes[0] === "once");
}

/** What each part of the review is, in the order it happens (the worktree is created first). */
export function requestTitle(r: ApprovalRequest): string {
  return r.class === "create_worktree" ? "Create a separate worktree" : r.class === "launch_agent" ? "Start the agent" : r.summary;
}

export const ordered = (requests: ApprovalRequest[]): ApprovalRequest[] => [...requests].sort((a, b) => Number(b.class === "create_worktree") - Number(a.class === "create_worktree"));
