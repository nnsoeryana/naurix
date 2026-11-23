import { describe, expect, it } from "vitest";
import { insertTarget, insertText, pasteRefusal } from "./model";

describe("insertTarget", () => {
  const kinds = { a: { type: "agent", agentId: "x", name: "X" }, s: { type: "shell" } } as never;
  const running = { a: { kind: "running" }, s: { kind: "running" } } as never;
  it("accepts only a running agent pane", () => {
    expect(insertTarget("a", kinds, running)).toEqual({ ok: true, paneId: "a" });
    expect(insertTarget("s", kinds, running).ok).toBe(false);
    expect(insertTarget(undefined, kinds, running).ok).toBe(false);
    expect(insertTarget("a", kinds, { a: { kind: "exited", code: 0, signal: null } } as never).ok).toBe(false);
    expect(insertTarget("a", kinds, {} as never).ok).toBe(false);
  });
  it("explains a refused shell in terms of the risk", () => {
    const r = insertTarget("s", kinds, running);
    expect(!r.ok && r.reason).toMatch(/never into a shell/);
  });
});

describe("insertText", () => {
  it("never ends in a newline, so Enter is never sent", () => {
    expect(insertText({ title: "T", body: "line\n\n" })).toBe("T\nline");
    expect(insertText({ title: "T", body: "" })).toBe("T");
    expect(insertText({ title: "T", body: "b" }).endsWith("\n")).toBe(false);
  });
});

describe("pasteRefusal", () => {
  it("refuses several lines unless bracketed paste is on, and always allows one line", () => {
    expect(pasteRefusal("a\nb", false)).toMatch(/every line break would act as Enter/);
    expect(pasteRefusal("a\nb", true)).toBeNull();
    expect(pasteRefusal("one line", false)).toBeNull();
  });
});
