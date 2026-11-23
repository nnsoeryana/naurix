import { describe, expect, it } from "vitest";
import type { Run, RunStep } from "./api";
import { addStep, canMarkDone, canSkip, draftOf, emptyDraft, isBlocked, removeStep, stepLabel, toggleDependency } from "./model";

const step = (id: string, state: RunStep["state"], deps: string[] = [], extra: Partial<RunStep> = {}): RunStep => ({ id, position: 0, name: id, agentId: "a", state, dependsOn: deps, startedAt: null, finishedAt: null, exitCode: null, note: null, ...extra });
const run = (steps: RunStep[], state: Run["state"] = "running"): Run => ({ id: "r", workflowId: null, workspaceId: "w", name: "r", state, createdAt: "", finishedAt: null, steps, sessions: {} });

describe("draft editing", () => {
  it("only earlier steps can be depended on", () => {
    let d = addStep(addStep(emptyDraft("claude-code"), "claude-code"), "claude-code");
    d = toggleDependency(d, 2, 0);
    d = toggleDependency(d, 2, 1);
    expect(d.steps[2].dependsOn).toEqual([0, 1]);
    expect(toggleDependency(d, 1, 2)).toBe(d); // a later step: refused, so no cycle can be drawn
    expect(toggleDependency(d, 1, 1)).toBe(d);
    expect(toggleDependency(d, 2, 0).steps[2].dependsOn).toEqual([1]); // toggles off
  });

  it("removing a step drops its edges and renumbers the rest", () => {
    let d = addStep(addStep(emptyDraft("x"), "x"), "x");
    d = toggleDependency(toggleDependency(d, 2, 0), 2, 1);
    d = removeStep(d, 0);
    expect(d.steps).toHaveLength(2);
    expect(d.steps[1].dependsOn).toEqual([0]);
  });

  it("a saved workflow comes back as positions", () => {
    const d = draftOf({ id: "w", workspaceId: "ws", name: "n", createdAt: "", updatedAt: "", steps: [{ id: "a", name: "a", agentId: "x", dependsOn: [] }, { id: "b", name: "b", agentId: "x", dependsOn: ["a"] }] });
    expect(d.steps[1].dependsOn).toEqual([0]);
  });
});

describe("run display", () => {
  it("a step behind a failed one is blocked, behind a running or skipped one it is only waiting", () => {
    const r = run([step("a", "failed"), step("b", "pending", ["a"]), step("c", "pending", ["b"]), step("d", "running"), step("e", "pending", ["d"]), step("f", "skipped"), step("g", "pending", ["f"])]);
    const blocked = (id: string) => isBlocked(r, r.steps.find((s) => s.id === id)!);
    expect([blocked("b"), blocked("c"), blocked("e"), blocked("g")]).toEqual([true, true, false, false]);
    expect(stepLabel(r, r.steps[1]).tone).toBe("warn");
    expect(stepLabel(r, r.steps[4]).text).toBe("Waiting");
  });

  it("labels say what happened and never invent output", () => {
    const r = run([]);
    expect(stepLabel(r, step("a", "succeeded", [], { exitCode: 0 })).text).toBe("Succeeded (exit 0)");
    expect(stepLabel(r, step("a", "succeeded")).text).toBe("Marked done");
    expect(stepLabel(r, step("a", "failed", [], { exitCode: 3, note: "exited with an error" })).text).toBe("Failed: exited with an error (exit 3)");
  });

  it("mark done and skip are offered only where the backend accepts them", () => {
    const r = run([step("a", "running"), step("b", "pending")]);
    expect([canMarkDone(r, r.steps[0]), canMarkDone(r, r.steps[1]), canSkip(r, r.steps[0]), canSkip(r, r.steps[1])]).toEqual([true, false, false, true]);
    const ended = run(r.steps, "cancelled");
    expect([canMarkDone(ended, ended.steps[0]), canSkip(ended, ended.steps[1])]).toEqual([false, false]);
  });
});
