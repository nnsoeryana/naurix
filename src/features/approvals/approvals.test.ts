import { describe, expect, it } from "vitest";
import { describeScope, scopeLabel } from "./api";
import { launchErrorMessage } from "../agents/api";

describe("approval wording", () => {
  it("labels every scope the backend can offer", () => {
    expect(scopeLabel("once")).toBe("Allow once");
    expect(scopeLabel("workspace")).toBe("Allow in this workspace");
    expect(scopeLabel("app")).toBe("Always allow this program");
  });

  it("describes a stored grant's reach, with or without a known workspace", () => {
    expect(describeScope("app", null)).toBe("Every workspace");
    expect(describeScope("workspace", "naurix")).toBe("Workspace: naurix");
    expect(describeScope("workspace", null)).toBe("One workspace");
    expect(describeScope("once", null)).toBe("Once");
  });
});

describe("launchErrorMessage", () => {
  const request = { id: "req-9f3a", class: "launch_agent", workspaceId: "w", summary: "s", display: "/opt/bin/agent --distinctive-flag", allowedScopes: ["once"], expiresInSecs: 300 };
  it("explains a missing approval without leaking the request", () => {
    const m = launchErrorMessage({ kind: "approval_required", request });
    expect(m).toMatch(/Approval is required/);
    expect(m).not.toContain("req-9f3a");
    expect(m).not.toContain("distinctive-flag");
  });
  it("passes a backend message through and stringifies anything else", () => {
    expect(launchErrorMessage({ kind: "error", message: "Codex was not found." })).toBe("Codex was not found.");
    expect(launchErrorMessage("plain")).toBe("plain");
    expect(launchErrorMessage(new Error("boom"))).toContain("boom");
  });
});
