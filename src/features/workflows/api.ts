import { invoke } from "@tauri-apps/api/core";
import type { ApprovalRequest } from "../approvals/api";

/** Workflows and runs (ADR 0030). Metadata only: ids, the user's own names, states and times. */
export type Step = { id: string; name: string; agentId: string; dependsOn: string[] };
export type Workflow = { id: string; workspaceId: string; name: string; steps: Step[]; createdAt: string; updatedAt: string };

export type StepState = "pending" | "running" | "succeeded" | "failed" | "skipped" | "cancelled";
export type RunState = "running" | "succeeded" | "failed" | "cancelled" | "interrupted";

export type RunStep = {
  id: string;
  position: number;
  name: string;
  agentId: string;
  state: StepState;
  dependsOn: string[];
  startedAt: string | null;
  finishedAt: string | null;
  exitCode: number | null;
  /** A short fixed reason (never process output). */
  note: string | null;
};

export type Run = {
  id: string;
  workflowId: string | null;
  workspaceId: string;
  name: string;
  state: RunState;
  createdAt: string;
  finishedAt: string | null;
  steps: RunStep[];
  /** The live session of each running step, by step id. */
  sessions: Record<string, number>;
};

/** What the editor sends. Dependencies are positions in the same list, and only earlier ones are offered. */
export type StepDraft = { name: string; agentId: string; dependsOn: number[] };

export type RunStart = { kind: "required"; request: ApprovalRequest } | { kind: "started"; run: Run };

export const workflowApi = {
  list: (workspaceId: string) => invoke<Workflow[]>("workflow_list", { workspaceId }),
  save: (workspaceId: string, id: string | null, name: string, steps: StepDraft[]) => invoke<Workflow>("workflow_save", { workspaceId, id, name, steps }),
  remove: (id: string) => invoke<void>("workflow_delete", { id }),
  run: (workflowId: string) => invoke<RunStart>("workflow_run", { workflowId }),
  runs: (workspaceId: string) => invoke<Run[]>("run_list", { workspaceId }),
  markDone: (runId: string, stepId: string) => invoke<Run>("run_mark_done", { runId, stepId }),
  skip: (runId: string, stepId: string) => invoke<Run>("run_skip", { runId, stepId }),
  cancel: (runId: string) => invoke<Run>("run_cancel", { runId }),
};
