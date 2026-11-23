import { describe, expect, it } from "vitest";
import type { Run, RunStep } from "./api";
import { edgesOf, graphHeight, layoutSteps, NODE_H, NODE_W } from "./graph";

const s = (id: string, dependsOn: string[] = [], state: RunStep["state"] = "pending"): RunStep => ({ id, position: 0, name: id, agentId: "a", state, dependsOn, startedAt: null, finishedAt: null, exitCode: null, note: null });

describe("layoutSteps", () => {
  it("puts a step one column right of its deepest dependency", () => {
    const p = layoutSteps([s("a"), s("b", ["a"]), s("c", ["a"]), s("d", ["b", "c"]), s("e", ["a", "d"])]);
    const col = (id: string) => p.get(id)!.x / (NODE_W + 70);
    expect([col("a"), col("b"), col("c"), col("d"), col("e")]).toEqual([0, 1, 1, 2, 3]);
  });

  it("keeps order within a column and centres short columns on the tallest one", () => {
    const p = layoutSteps([s("a"), s("b", ["a"]), s("c", ["a"]), s("d", ["b", "c"])]);
    expect(p.get("b")!.y).toBeLessThan(p.get("c")!.y);
    // a and d are alone in their columns: centred between b and c.
    const mid = (p.get("b")!.y + p.get("c")!.y) / 2;
    expect(p.get("a")!.y).toBe(mid);
    expect(p.get("d")!.y).toBe(mid);
  });

  it("never throws or loops on a missing dependency or a cycle, and places every step once", () => {
    const p = layoutSteps([s("a", ["x"]), s("b", ["c"]), s("c", ["b"])]);
    expect([...p.keys()].sort()).toEqual(["a", "b", "c"]);
    for (const pt of p.values()) expect(Number.isFinite(pt.x + pt.y)).toBe(true);
  });

  it("an independent step starts in the first column", () => {
    const p = layoutSteps([s("a"), s("b")]);
    expect([p.get("a")!.x, p.get("b")!.x]).toEqual([0, 0]);
    expect(p.get("b")!.y).toBeGreaterThanOrEqual(p.get("a")!.y + NODE_H);
  });
});

describe("graphHeight and edges", () => {
  it("stays within bounds for one and for many parallel steps", () => {
    expect(graphHeight([s("a")])).toBe(180);
    expect(graphHeight(Array.from({ length: 24 }, (_, i) => s(`n${i}`)))).toBe(480);
  });

  it("makes one edge per dependency and ignores unknown ones", () => {
    const run = { steps: [s("a"), s("b", ["a", "gone"], "running")] } as Run;
    const e = edgesOf(run, (x) => x.id === "b");
    expect(e).toEqual([{ id: "a->b", source: "a", target: "b", animated: true, blocked: true }]);
  });
});
