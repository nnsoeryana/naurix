export type NauriMode = "idle" | "ready" | "working" | "waiting" | "success" | "error";

/** How Naurix may move. `paused` = animated but the window is hidden. */
export type MotionLevel = "off" | "reduced" | "full" | "paused";

export type NauriSummary = {
  /** Steady state derived from agent sessions (never "success": that is a timed event). */
  mode: Exclude<NauriMode, "success">;
  /** Agent processes that are starting or running. */
  running: number;
  /** Of those, how many are mid-turn (output flowing after the user's input). */
  busy: number;
  /** Agents that failed or exited with a non-zero code. */
  failed: number;
  /** Agents with a reliable "needs input" signal. No adapter provides one yet, so this is always 0. */
  waiting: number;
};
