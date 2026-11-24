import { invoke } from "@tauri-apps/api/core";

/** Opt-in project memory (ADR 0031): text the user typed, per workspace. Never captured from anywhere. */
export type Note = { id: string; workspaceId: string; title: string; body: string; createdAt: string; updatedAt: string };
export type MemoryStatus = { enabled: boolean; count: number };
export type ExportResult = { kind: "cancelled" } | { kind: "written"; path: string; inRepository: boolean };

export const MAX_TITLE = 100;
export const MAX_BODY = 8000;

export const memoryApi = {
  status: (workspaceId: string) => invoke<MemoryStatus>("memory_status", { workspaceId }),
  setEnabled: (workspaceId: string, enabled: boolean) => invoke<MemoryStatus>("memory_set_enabled", { workspaceId, enabled }),
  list: (workspaceId: string, query: string | null) => invoke<Note[]>("memory_list", { workspaceId, query }),
  save: (workspaceId: string, id: string | null, title: string, body: string) => invoke<Note>("memory_save", { workspaceId, id, title, body }),
  remove: (workspaceId: string, id: string) => invoke<void>("memory_delete", { workspaceId, id }),
  removeAll: (workspaceId: string) => invoke<number>("memory_delete_all", { workspaceId }),
  export: (workspaceId: string, format: "markdown" | "json") => invoke<ExportResult>("memory_export", { workspaceId, format }),
};
