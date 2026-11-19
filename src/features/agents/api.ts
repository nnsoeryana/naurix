import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ExitPayload } from "../terminal/api";

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
  /** Already reviewed for this exact executable. */
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

export const agentApi = {
  list: () => invoke<AgentInfo[]>("agent_list"),
  setPath: (id: string, path: string | null) => invoke<AgentInfo[]>("agent_set_path", { id, path }),
  saveCustom: (agent: CustomAgentInput) => invoke<AgentInfo[]>("agent_custom_save", { agent }),
  deleteCustom: (id: string) => invoke<AgentInfo[]>("agent_custom_delete", { id }),
  launch: (workspaceId: string, agentId: string, reviewed: boolean, rows: number, cols: number, onOutput: Channel<ArrayBuffer>, onExit: Channel<ExitPayload>) =>
    invoke<number>("agent_launch", { workspaceId, agentId, reviewed, rows, cols, onOutput, onExit }),
  /** Reported version; only for an executable you have already approved (it runs `--version`). */
  version: (id: string) => invoke<string | null>("agent_version", { id }),
  pickExecutable: async (): Promise<string | null> => {
    const r = await open({ directory: false, multiple: false, title: "Choose the agent executable" });
    return typeof r === "string" ? r : null;
  },
};
