import type { Run, RunStep } from "./api";

/** Size and spacing of a step node, in canvas pixels. */
export const NODE_W = 220;
export const NODE_H = 92;
const GAP_X = 70;
const GAP_Y = 22;

type Pt = { x: number; y: number };

/**
 * Layered layout without a library: a step sits one column to the right of its deepest dependency, and
 * steps in a column keep their order and are centred vertically. Steps are at most 24, so this is cheap.
 * A dependency that is missing or forms a loop (the backend refuses those) counts as depth 0.
 */
export function layoutSteps(steps: Pick<RunStep, "id" | "dependsOn">[]): Map<string, Pt> {
  const by = new Map(steps.map((s) => [s.id, s]));
  const depth = new Map<string, number>();
  const visiting = new Set<string>();
  const depthOf = (id: string): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    if (visiting.has(id)) return 0;
    visiting.add(id);
    const s = by.get(id);
    const d = s ? Math.max(-1, ...s.dependsOn.filter((x) => by.has(x)).map(depthOf)) + 1 : 0;
    visiting.delete(id);
    depth.set(id, d);
    return d;
  };
  const columns = new Map<number, string[]>();
  for (const s of steps) {
    const d = depthOf(s.id);
    columns.set(d, [...(columns.get(d) ?? []), s.id]);
  }
  const tallest = Math.max(1, ...[...columns.values()].map((c) => c.length));
  const out = new Map<string, Pt>();
  for (const [d, ids] of columns) {
    const offset = ((tallest - ids.length) * (NODE_H + GAP_Y)) / 2;
    ids.forEach((id, row) => out.set(id, { x: d * (NODE_W + GAP_X), y: offset + row * (NODE_H + GAP_Y) }));
  }
  return out;
}

/** The canvas height that fits the tallest column, kept within a sensible range. */
export function graphHeight(steps: Pick<RunStep, "id" | "dependsOn">[]): number {
  const pos = layoutSteps(steps);
  const bottom = Math.max(0, ...[...pos.values()].map((p) => p.y + NODE_H));
  return Math.min(480, Math.max(180, bottom + 48));
}

export type EdgeSpec = { id: string; source: string; target: string; animated: boolean; blocked: boolean };

/** One edge per dependency. An edge into a running step is animated; one into a blocked step is dashed. */
export function edgesOf(run: Run, isBlocked: (s: RunStep) => boolean): EdgeSpec[] {
  const by = new Map(run.steps.map((s) => [s.id, s]));
  return run.steps.flatMap((s) =>
    s.dependsOn
      .filter((d) => by.has(d))
      .map((d) => ({ id: `${d}->${s.id}`, source: d, target: s.id, animated: s.state === "running", blocked: isBlocked(s) })),
  );
}
