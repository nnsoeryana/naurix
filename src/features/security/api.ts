import { invoke } from "@tauri-apps/api/core";

/**
 * Security Suite (ADR 0032). A report is read when the user picks it in the OS dialog (the backend opens
 * the dialog; this page never names a path), shown, and forgotten. Nothing here is stored.
 */
export type Severity = "critical" | "high" | "medium" | "low" | "info";
export type Level = "error" | "warning" | "note" | "none";

export type Finding = {
  tool: string;
  ruleId: string;
  severity: Severity;
  level: Level;
  /** Relative to the project folder; null when the report gave none or one outside the project. */
  location: { path: string; line: number | null } | null;
  message: string;
  fingerprint: string;
};

export type Report = {
  tools: string[];
  findings: Finding[];
  /** Results over the limit: counted, not shown. */
  notShown: number;
  /** Results whose path was outside the project or unusable (shown without a location). */
  pathsDropped: number;
  /** Messages hidden because they looked like they held a secret. */
  messagesHidden: number;
};

export type Opened = { fileName: string; report: Report };

export type Comparison = {
  new: Finding[];
  stillPresent: Finding[];
  /** The scanner stopped reporting these. Not the same as fixed. */
  noLongerReported: Finding[];
  /** A report was over the limit, so the lists may be wrong. */
  incomplete: boolean;
};

export type OpenResult = { kind: "cancelled" } | ({ kind: "opened" } & Opened);
export type CompareResult = { kind: "cancelled" } | { kind: "compared"; baseline: Opened; current: Opened; comparison: Comparison };

export const securityApi = {
  openReport: (workspaceId: string) => invoke<OpenResult>("security_open_report", { workspaceId }),
  compare: (workspaceId: string) => invoke<CompareResult>("security_compare", { workspaceId }),
};
