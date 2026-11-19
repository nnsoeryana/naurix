import { describe, expect, it } from "vitest";
import type { TerminalState } from "../terminal/TerminalView";
import type { PaneKind } from "../../state/store";
import { THEMES } from "../../themes/registry";
import { cssVariables } from "../../themes/apply";
import { resolveAppearance } from "../../themes/resolve";
import { DEFAULT_SETTINGS } from "../settings/api";
import { detectCompletions, displayMode, motionLevel, statusText, summarize } from "./state";

const agent: PaneKind = { type: "agent", agentId: "a", name: "A", reviewed: true };
const shell: PaneKind = { type: "shell" };
const run: TerminalState = { kind: "running" };
const ok: TerminalState = { kind: "exited", code: 0, signal: null };
const bad: TerminalState = { kind: "exited", code: 2, signal: null };
const killed: TerminalState = { kind: "exited", code: null, signal: "Killed: 9" };

describe("summarize", () => {
  it("is ready when agents are alive but none is mid-turn, working when one is", () => {
    const k = { a: agent, b: agent };
    expect(summarize({ a: run, b: run }, k, undefined, new Set()).mode).toBe("ready");
    const s = summarize({ a: run, b: run }, k, undefined, new Set(["b"]));
    expect(s.mode).toBe("working");
    expect(s.busy).toBe(1);
    expect(summarize({}, {}, undefined, new Set(["x"])).mode).toBe("idle");
  });
  it("is idle with no agent, and ignores shells", () => {
    expect(summarize({}, {}).mode).toBe("idle");
    expect(summarize({ s: run }, { s: shell }).mode).toBe("idle");
  });
  it("is working while an agent starts or runs", () => {
    expect(summarize({}, { a: agent }).mode).toBe("working");
    expect(summarize({ a: { kind: "starting" } }, { a: agent }).mode).toBe("working");
    expect(summarize({ a: run, b: run, c: run }, { a: agent, b: agent, c: agent }).running).toBe(3);
  });
  it("errors on failed or non-zero exit, but not on clean exit or a stop by signal", () => {
    expect(summarize({ a: { kind: "failed", message: "x" } }, { a: agent }).mode).toBe("error");
    expect(summarize({ a: bad }, { a: agent }).mode).toBe("error");
    expect(summarize({ a: ok }, { a: agent }).mode).toBe("idle");
    expect(summarize({ a: killed }, { a: agent }).mode).toBe("idle");
  });
  it("priority: waiting > error > working > idle", () => {
    const kinds = { a: agent, b: agent, c: agent };
    expect(summarize({ a: run, b: bad }, kinds).mode).toBe("error");
    expect(summarize({ a: run, b: bad, c: run }, kinds, new Set(["c"])).mode).toBe("waiting");
  });
  it("never reports waiting without an explicit signal", () => {
    expect(summarize({ a: run }, { a: agent }).waiting).toBe(0);
  });
});

describe("success events", () => {
  const kinds = { a: agent, s: shell };
  it("fires once for running -> clean exit of an agent", () => {
    expect(detectCompletions({ a: run }, { a: ok }, kinds)).toEqual(["a"]);
    expect(detectCompletions({ a: ok }, { a: ok }, kinds)).toEqual([]);
  });
  it("does not fire for a stop, a failure, a shell, or a never-seen-running pane (restored state)", () => {
    expect(detectCompletions({ a: run }, { a: killed }, kinds)).toEqual([]);
    expect(detectCompletions({ a: run }, { a: bad }, kinds)).toEqual([]);
    expect(detectCompletions({ s: run }, { s: ok }, kinds)).toEqual([]);
    expect(detectCompletions({}, { a: ok }, kinds)).toEqual([]);
    expect(detectCompletions({ a: { kind: "starting" } }, { a: ok }, kinds)).toEqual([]);
  });
});

describe("display and text", () => {
  const idle = summarize({}, {});
  it("success is temporary and outranked by error/waiting", () => {
    expect(displayMode(idle, true)).toBe("success");
    expect(displayMode(idle, false)).toBe("idle");
    expect(displayMode(summarize({ a: bad }, { a: agent }), true)).toBe("error");
  });
  it("summarizes several agents without claiming task state", () => {
    const s = summarize({ a: run, b: run, c: run }, { a: agent, b: agent, c: agent });
    expect(statusText("working", s)).toBe("Waiting for 3 agents...");
    for (const m of ["idle", "working", "waiting", "success", "error"] as const) expect(statusText(m, s)).toBeTruthy();
  });
});

describe("motion", () => {
  const base = { animations: true, reduced: false, systemReduced: false, hidden: false };
  it("respects settings, system reduced motion and visibility", () => {
    expect(motionLevel(base)).toBe("full");
    expect(motionLevel({ ...base, reduced: true })).toBe("reduced");
    expect(motionLevel({ ...base, animations: false })).toBe("off");
    expect(motionLevel({ ...base, systemReduced: true })).toBe("off");
    expect(motionLevel({ ...base, hidden: true })).toBe("paused");
  });
  it("is on by default", () => {
    const g = DEFAULT_SETTINGS.general;
    expect([g.nauriEnabled, g.nauriAnimations, g.nauriReduced]).toEqual([true, true, false]);
  });
});

describe("theme compatibility", () => {
  for (const t of THEMES) {
    it(`${t.id} defines every color Naurix uses`, () => {
      const vars = cssVariables(resolveAppearance({ mode: t.kind, darkTheme: t.id, lightTheme: t.id, accent: null }, t.kind === "dark"));
      for (const v of ["--color-terminal", "--color-purple", "--color-teal", "--color-info", "--color-warning", "--color-muted", "--color-line"]) expect(vars[v], `${t.id} ${v}`).toMatch(/^#[0-9a-f]{6}$/i);
    });
  }
});
