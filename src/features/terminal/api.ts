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
};

/** A session Core still knows about: running, or exited but not yet closed (ADR 0026). Metadata only. */
export type LiveSession = {
  id: number;
  workspaceId: string;
  kind: "terminal" | "agent";
  title: string;
  adapterId: string | null;
  startedAt: string;
  running: boolean;
  code: number | null;
  signal: string | null;
};

/**
 * attach and detach for one session run strictly in the order they were issued. Tauri runs async
 * commands concurrently, and under React StrictMode (or hot reload) a pane is mounted, cleaned up and
 * mounted again, so an old cleanup must never overtake a newer attach.
 */
const chains = new Map<number, Promise<unknown>>();
function serial<T>(id: number, run: () => Promise<T>): Promise<T> {
  const next = (chains.get(id) ?? Promise.resolve()).catch(() => {}).then(run);
  chains.set(id, next.catch(() => {}));
  return next;
}

export const sessionApi = {
  list: () => invoke<LiveSession[]>("session_list"),
  /** Replays the buffered tail, then streams live output. Resolves to the consumer generation for `detach`. */
  attach: (id: number, rows: number, cols: number, onOutput: Channel<ArrayBuffer>, onExit: Channel<ExitPayload>) =>
    serial(id, () => invoke<number>("session_attach", { id, rows, cols, onOutput, onExit })),
  /** This view is going away. The process keeps running. Ignored if a newer view has attached since. */
  detach: (id: number, generation: number) => serial(id, () => invoke<void>("session_detach", { id, generation })),
  /** The user closed the pane: stop the process if it runs and forget the session. */
  close: (id: number) => invoke<void>("session_close", { id }),
};
