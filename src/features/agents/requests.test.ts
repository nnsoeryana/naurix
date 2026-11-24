import { describe, expect, it } from "vitest";
import { extrasAreOnceOnly, ordered, requestTitle, splitRequests } from "./requests";
import type { ApprovalRequest } from "../approvals/api";

const req = (over: Partial<ApprovalRequest>): ApprovalRequest => ({ id: "r", class: "launch_agent", workspaceId: "w", summary: "s", display: "d", allowedScopes: ["once", "workspace"], expiresInSecs: 300, ...over });
const launch = req({ id: "L" });
const create = req({ id: "C", class: "create_worktree", allowedScopes: ["once"] });

describe("launch requests", () => {
  it("picks the agent request as the one the user chooses a scope for", () => {
    const s = splitRequests([create, launch])!;
    expect(s.main.id).toBe("L");
    expect(s.also.map((r) => r.id)).toEqual(["C"]);
  });

  it("falls back to the first request when only the worktree needs a decision", () => {
    const s = splitRequests([create])!;
    expect(s.main.id).toBe("C");
    expect(s.also).toEqual([]);
    expect(splitRequests([])).toBeNull();
  });

  it("only approves extras for the user when they can ONLY ever be approved once", () => {
    expect(extrasAreOnceOnly([create])).toBe(true);
    expect(extrasAreOnceOnly([])).toBe(true);
    expect(extrasAreOnceOnly([req({ id: "X", allowedScopes: ["once", "workspace"] })])).toBe(false);
  });

  it("shows the worktree first because it happens first, and names each part", () => {
    expect(ordered([launch, create]).map((r) => r.id)).toEqual(["C", "L"]);
    expect(requestTitle(create)).toBe("Create a separate worktree");
    expect(requestTitle(launch)).toBe("Start the agent");
    expect(requestTitle(req({ class: "other", summary: "Something" }))).toBe("Something");
  });
});
