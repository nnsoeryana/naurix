import type { Run, RunStep, StepDraft, Workflow } from "./api";

/** A workflow as the editor holds it. */
export type Draft = { id: string | null; name: string; steps: StepDraft[] };

export const emptyDraft = (agentId: string): Draft => ({ id: null, name: "", steps: [{ name: "Step 1", agentId, dependsOn: [] }] });

export function draftOf(w: Workflow): Draft {
  return {
    id: w.id,
    name: w.name,
    steps: w.steps.map((s) => ({ name: s.name, agentId: s.agentId, dependsOn: s.dependsOn.map((d) => w.steps.findIndex((x) => x.id === d)).filter((i) => i >= 0) })),
  };
}

export function addStep(d: Draft, agentId: string): Draft {
  return { ...d, steps: [...d.steps, { name: `Step ${d.steps.length + 1}`, agentId, dependsOn: [] }] };
}

/** Removing a step drops the edges that pointed at it and renumbers the ones after it. */
export function removeStep(d: Draft, index: number): Draft {
  return {
    ...d,
    steps: d.steps
      .filter((_, i) => i !== index)
      .map((s) => ({ ...s, dependsOn: s.dependsOn.filter((x) => x !== index).map((x) => (x > index ? x - 1 : x)) })),
  };
}

/** Only earlier steps can be depended on, so an edited workflow can never contain a cycle. */
export function toggleDependency(d: Draft, step: number, on: number): Draft {
  if (on >= step) return d;
  return {
    ...d,
    steps: d.steps.map((s, i) => (i !== step ? s : { ...s, dependsOn: s.dependsOn.includes(on) ? s.dependsOn.filter((x) => x !== on) : [...s.dependsOn, on].sort((a, b) => a - b) })),
  };
}

/** A pending step that can never start because something it needs failed or was cancelled. */
export function isBlocked(run: Run, step: RunStep): boolean {
  if (step.state !== "pending") return false;
  const by = new Map(run.steps.map((s) => [s.id, s]));
  const stuck = (s: RunStep, seen: Set<string>): boolean =>
    s.dependsOn.some((d) => {
      const x = by.get(d);
      if (!x || seen.has(x.id)) return false;
      seen.add(x.id);
      return x.state === "failed" || x.state === "cancelled" || (x.state === "pending" && stuck(x, seen));
    });
  return stuck(step, new Set());
}

export type StepLabel = { text: string; tone: "ok" | "bad" | "warn" | "idle" | "live" };

export function stepLabel(run: Run, s: RunStep): StepLabel {
  switch (s.state) {
    case "running":
      return { text: "Running", tone: "live" };
    case "succeeded":
      return { text: s.exitCode === 0 ? "Succeeded (exit 0)" : "Marked done", tone: "ok" };
    case "failed":
      return { text: `Failed${s.note ? `: ${s.note}` : ""}${s.exitCode !== null ? ` (exit ${s.exitCode})` : ""}`, tone: "bad" };
    case "skipped":
      return { text: "Skipped", tone: "idle" };
    case "cancelled":
      return { text: "Cancelled", tone: "idle" };
    case "pending":
      return isBlocked(run, s) ? { text: "Blocked: a step before it did not succeed", tone: "warn" } : { text: "Waiting", tone: "idle" };
  }
}

export const canMarkDone = (run: Run, s: RunStep): boolean => run.state === "running" && s.state === "running";
export const canSkip = (run: Run, s: RunStep): boolean => run.state === "running" && s.state === "pending";
