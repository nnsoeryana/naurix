import { expect, test } from "vitest";
import { agentStatus } from "./status";

test("only process states are ever reported", () => {
  const all = [
    agentStatus(undefined),
    agentStatus({ kind: "starting" }),
    agentStatus({ kind: "running" }),
    agentStatus({ kind: "exited", code: 0, signal: null }),
    agentStatus({ kind: "failed", message: "boom" }),
  ];
  expect(new Set(all.map((s) => s.label))).toEqual(new Set(["Starting", "Running", "Exited", "Failed"]));
  for (const s of all) expect(`${s.label} ${s.detail}`).not.toMatch(/waiting|approval|approve|completed|working|thinking|idle/i);
});

test("running explicitly says task state is unknown", () => {
  expect(agentStatus({ kind: "running" }).detail).toMatch(/not reported/);
});

test("exit details", () => {
  expect(agentStatus({ kind: "exited", code: 2, signal: null }).detail).toBe("Exited with code 2.");
  expect(agentStatus({ kind: "exited", code: null, signal: "Killed: 9" }).detail).toBe("Ended by signal Killed: 9.");
});

test("a reported signal is shown only while the process runs, with its time, as a report", () => {
  const at = "2026-11-23T10:00:00Z";
  const run = { kind: "running" } as const;
  expect(agentStatus(run, { kind: "awaiting_approval", at }).label).toBe("Waiting for approval");
  expect(agentStatus(run, { kind: "idle", at }).label).toBe("Waiting for input");
  expect(agentStatus(run, { kind: "turn_done", at }).label).toBe("Turn finished");
  expect(agentStatus(run, { kind: "working", at }).label).toBe("Working");
  expect(agentStatus(run, { kind: "working", at }).detail).toMatch(/last report, not a live check/);
  // After the process ended, or before it started, a stale signal is ignored.
  expect(agentStatus({ kind: "exited", code: 0, signal: null }, { kind: "working", at }).label).toBe("Exited");
  expect(agentStatus({ kind: "starting" }, { kind: "working", at }).label).toBe("Starting");
  expect(agentStatus({ kind: "failed", message: "x" }, { kind: "idle", at }).label).toBe("Failed");
});
