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
