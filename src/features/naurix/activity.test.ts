import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { endTurn, getBusyPanes, noteOutput, QUIET_MS, startTurn } from "./activity";

describe("turn activity", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal("window", globalThis); // node test env has no window
  });
  afterEach(() => {
    endTurn("a");
    endTurn("b");
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("is busy after startTurn and quiet after QUIET_MS without output", () => {
    startTurn("a");
    expect(getBusyPanes().has("a")).toBe(true);
    vi.advanceTimersByTime(QUIET_MS + 1);
    expect(getBusyPanes().has("a")).toBe(false);
  });

  it("output extends a turn, but output outside a turn is ignored", () => {
    noteOutput("b");
    expect(getBusyPanes().has("b")).toBe(false);
    startTurn("a");
    vi.advanceTimersByTime(QUIET_MS - 100);
    noteOutput("a");
    vi.advanceTimersByTime(QUIET_MS - 100);
    expect(getBusyPanes().has("a")).toBe(true);
  });

  it("endTurn clears immediately", () => {
    startTurn("a");
    endTurn("a");
    expect(getBusyPanes().has("a")).toBe(false);
  });
});
