export type NauriMode = "idle" | "working" | "waiting" | "success" | "error";

/** How Nauri may move. `paused` = animated but the window is hidden. */
export type MotionLevel = "off" | "reduced" | "full" | "paused";

export type NauriSummary = {
  /** Steady state derived from agent sessions (never "success": that is a timed event). */
  mode: Exclude<NauriMode, "success">;
  /** Agent processes that are starting or running. */
  running: number;
  /** Agents that failed or exited with a non-zero code. */
  failed: number;
  /** Agents with a reliable "needs input" signal. No adapter provides one yet, so this is always 0. */
  waiting: number;
};
