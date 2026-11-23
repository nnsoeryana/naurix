import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ExitPayload } from "../terminal/api";
import type { ApprovalRequest } from "../approvals/api";

export type Capability = {
  feature: "pty_launch" | "mcp_config" | "skills" | "status_signals" | "session_resume";
  level: "verified" | "documented_only" | "documented_accepted" | "unsupported";
  evidence?: string;
  basis?: string;
  source?: string;
  reason?: string;
  detail: string;
};

export type AdapterInfo = { capabilities: Capability[]; notice: string | null; verifiedWith: string | null };

export type AgentInfo = {
  id: string;
  name: string;
  kind: "preset" | "custom";
  executable: string;
  args: string[];
  env: { name: string; value: string }[];
  /** Where the executable was found; null when not installed / not found. */
  resolvedPath: string | null;
  pathOverride: string | null;
  /** The user approved this exact executable for every workspace (presets only). */
  approved: boolean;
  command: string | null;
  capabilities: string[];
  adapter: AdapterInfo | null;
};

export type CustomAgentInput = {
  id?: string;
  name: string;
  executable: string;
  args: string[];
  env: { name: string; value: string }[];
};

export type AuthorizeResult = { kind: "allowed" } | { kind: "required"; request: ApprovalRequest };
export type LaunchError = { kind: "approval_required"; request: ApprovalRequest } | { kind: "error"; message: string };

/** A readable message for a failed launch, whatever shape the backend rejected with. */
export function launchErrorMessage(e: unknown): string {
  if (e && typeof e === "object" && "kind" in e) {
    const x = e as LaunchError;
    if (x.kind === "approval_required") return "Approval is required before this agent can start. Launch it again from the Agents panel.";
    if (x.kind === "error") return x.message;
  }
  return String(e);
}

export const agentApi = {
  list: () => invoke<AgentInfo[]>("agent_list"),
  setPath: (id: string, path: string | null) => invoke<AgentInfo[]>("agent_set_path", { id, path }),
  saveCustom: (agent: CustomAgentInput) => invoke<AgentInfo[]>("agent_custom_save", { agent }),
  deleteCustom: (id: string) => invoke<AgentInfo[]>("agent_custom_delete", { id }),
  /** Would this agent start now? Spends nothing. When approval is needed the answer carries the request to show. */
  authorize: (workspaceId: string, agentId: string) => invoke<AuthorizeResult>("agent_authorize", { workspaceId, agentId }),
  /** Fails with `{ kind: "approval_required" }` (starting nothing) unless a matching approval exists. */
  launch: (workspaceId: string, agentId: string, rows: number, cols: number, onOutput: Channel<ArrayBuffer>, onExit: Channel<ExitPayload>) =>
    invoke<number>("agent_launch", { workspaceId, agentId, rows, cols, onOutput, onExit }),
  /** Reported version; only for a preset whose executable you approved for every workspace (it runs `--version`). */
  version: (id: string) => invoke<string | null>("agent_version", { id }),
  pickExecutable: async (): Promise<string | null> => {
    const r = await open({ directory: false, multiple: false, title: "Choose the agent executable" });
    return typeof r === "string" ? r : null;
  },
};
