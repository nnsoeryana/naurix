import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export type Workspace = {
  id: string;
  name: string;
  rootPath: string;
  lastOpenedAt: string | null;
  available: boolean;
};

export type SessionRecord = {
  id: string;
  kind: string;
  title: string;
  lifecycle: string;
  startedAt: string | null;
  endedAt: string | null;
};

export const workspaceApi = {
  list: () => invoke<Workspace[]>("workspace_list"),
  create: (path: string) => invoke<Workspace>("workspace_create", { path }),
  rename: (id: string, name: string) => invoke<Workspace>("workspace_rename", { id, name }),
  open: (id: string) => invoke<void>("workspace_open", { id }),
  remove: (id: string) => invoke<void>("workspace_remove", { id }),
  recentSessions: (workspaceId: string) => invoke<SessionRecord[]>("session_recent", { workspaceId }),
  /** Native folder picker; resolves to null if the user cancels. */
  pickFolder: async (): Promise<string | null> => {
    const r = await open({ directory: true, multiple: false, title: "Open project folder" });
    return typeof r === "string" ? r : null;
  },
};
