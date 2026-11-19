import { Channel, invoke } from "@tauri-apps/api/core";

export type ExitPayload = { code: number | null; signal: string | null };

export const terminalApi = {
  spawn: (workspaceId: string, title: string, atRepoRoot: boolean, rows: number, cols: number, onOutput: Channel<ArrayBuffer>, onExit: Channel<ExitPayload>) =>
    invoke<number>("terminal_spawn", { workspaceId, title, atRepoRoot, rows, cols, onOutput, onExit }),
  hasForegroundJob: (id: number) => invoke<boolean>("terminal_has_foreground_job", { id }),
  write: (id: number, data: string) => invoke<void>("terminal_write", { id, data }),
  writeBytes: (id: number, data: number[]) => invoke<void>("terminal_write_bytes", { id, data }),
  resize: (id: number, rows: number, cols: number) => invoke<void>("terminal_resize", { id, rows, cols }),
  ack: (id: number, bytes: number) => invoke<void>("terminal_ack", { id, bytes }),
  terminate: (id: number) => invoke<void>("terminal_terminate", { id }),
};
