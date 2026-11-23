import { describe, expect, it } from "vitest";
import { planReattach } from "./reattach";
import type { LiveSession } from "./api";

const s = (over: Partial<LiveSession>): LiveSession => ({ id: 1, workspaceId: "w1", kind: "terminal", title: "~/proj", adapterId: null, worktreeBranch: null, startedAt: "2026-11-21T00:00:00Z", running: true, code: null, signal: null, ...over });

describe("planReattach", () => {
  it("returns one tab per session of a known workspace, oldest first", () => {
    const plan = planReattach([s({ id: 5 }), s({ id: 2 }), s({ id: 9, workspaceId: "w2" })], new Set(["w1", "w2"]));
    expect(plan.map((p) => p.sessionId)).toEqual([2, 5, 9]);
    expect(plan.map((p) => p.workspaceId)).toEqual(["w1", "w1", "w2"]);
  });

  it("restores agents as agent panes and shells as shell panes", () => {
    const [shell, agent] = planReattach([s({ id: 1 }), s({ id: 2, kind: "agent", title: "Claude Code", adapterId: "claude-code" })], new Set(["w1"]));
    expect(shell.kind).toEqual({ type: "shell", sessionId: 1 });
    expect(agent.kind).toEqual({ type: "agent", agentId: "claude-code", name: "Claude Code", sessionId: 2 });
  });

  it("keeps the branch of an agent that ran in a worktree", () => {
    const [a] = planReattach([s({ id: 3, kind: "agent", adapterId: "codex", title: "Codex", worktreeBranch: "naurix/codex-20261123-1105" })], new Set(["w1"]));
    expect(a.kind).toMatchObject({ type: "agent", branch: "naurix/codex-20261123-1105" });
  });

  it("skips sessions of workspaces that are not listed and keeps exited sessions so their output can be read", () => {
    const plan = planReattach([s({ id: 1, workspaceId: "gone" }), s({ id: 2, running: false, code: 1 })], new Set(["w1"]));
    expect(plan.map((p) => p.sessionId)).toEqual([2]);
  });

  it("does not modify its input", () => {
    const input = [s({ id: 3 }), s({ id: 1 })];
    planReattach(input, new Set(["w1"]));
    expect(input.map((x) => x.id)).toEqual([3, 1]);
  });
});
