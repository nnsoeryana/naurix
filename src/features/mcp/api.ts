import { invoke } from "@tauri-apps/api/core";

export type Transport = "stdio" | "http";
export type Pair = { name: string; value: string };

/** What the editor submits. Secret VALUES are never part of it (they go through `secretSet`). */
export type McpInput = {
  id?: string | null;
  name: string;
  transport: Transport;
  command: string | null;
  args: string[];
  url: string | null;
  env: Pair[];
  headers: Pair[];
  secrets: string[];
};

export type McpServer = {
  id: string;
  name: string;
  transport: Transport;
  command: string | null;
  args: string[];
  url: string | null;
  env: Pair[];
  headers: Pair[];
  secrets: { name: string; hasValue: boolean }[];
  enabled: boolean;
  approved: boolean;
  fingerprint: string;
  approvedAt: string | null;
  lastStatus: string | null;
  lastError: string | null;
  lastToolCount: number | null;
  lastTestedAt: string | null;
};

export type CatalogEntry = {
  id: string;
  title: string;
  description: string;
  source: string;
  transport: Transport;
  notes: string[];
};

// The core crate serializes these with snake_case keys.
export type ProbeResult = {
  ok: boolean;
  status: "connected" | "failed" | "not_testable";
  server_name: string | null;
  server_version: string | null;
  protocol: string | null;
  tools: { name: string; description: string | null }[];
  error: string | null;
};

export type PlannedChange = { entry_key: string; action: "add" | "update" | "remove" | "unchanged" | "conflict"; detail: string };
export type SyncPlan = {
  agent_id: string;
  file: string;
  file_exists: boolean;
  blocked: string | null;
  changes: PlannedChange[];
  plan_hash: string;
  preview: string;
  notes: string[];
};
export type SyncRun = { id: string; agent_id: string; config_path: string; summary: string; created_at: string; rolled_back_at: string | null };
export type SyncAgent = { agentId: string; name: string; supported: boolean; note: string };

export const mcpApi = {
  list: () => invoke<McpServer[]>("mcp_list"),
  catalog: () => invoke<CatalogEntry[]>("mcp_catalog"),
  catalogInput: (entryId: string, workspaceId: string | null) => invoke<McpInput>("mcp_catalog_input", { entryId, workspaceId }),
  save: (input: McpInput) => invoke<McpServer[]>("mcp_save", { input }),
  remove: (id: string) => invoke<McpServer[]>("mcp_delete", { id }),
  setEnabled: (id: string, enabled: boolean) => invoke<McpServer[]>("mcp_set_enabled", { id, enabled }),
  secretSet: (id: string, name: string, value: string) => invoke<McpServer[]>("mcp_secret_set", { id, name, value }),
  secretClear: (id: string, name: string) => invoke<McpServer[]>("mcp_secret_clear", { id, name }),
  approve: (id: string, fingerprint: string) => invoke<McpServer[]>("mcp_approve", { id, fingerprint }),
  test: (id: string) => invoke<ProbeResult>("mcp_test", { id }),
  syncAgents: () => invoke<SyncAgent[]>("mcp_sync_agents"),
  syncPlan: (workspaceId: string, agentId: string) => invoke<SyncPlan>("mcp_sync_plan", { workspaceId, agentId }),
  syncApply: (workspaceId: string, agentId: string, planHash: string) => invoke<SyncRun>("mcp_sync_apply", { workspaceId, agentId, planHash }),
  syncRuns: (agentId: string) => invoke<SyncRun[]>("mcp_sync_runs", { agentId }),
  syncRollback: (runId: string) => invoke<SyncRun>("mcp_sync_rollback", { runId }),
};

export function summarize(s: Pick<McpServer, "transport" | "command" | "args" | "url">): string {
  return s.transport === "http" ? (s.url ?? "") : [s.command, ...s.args].join(" ");
}
