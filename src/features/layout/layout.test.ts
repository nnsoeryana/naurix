import { expect, test } from "vitest";
import { cyclePane, hasPane, leaf, MAX_RATIO, MIN_RATIO, paneIds, removePane, setRatio, splitPane, type Layout } from "./layout";

const two = (): Layout => splitPane(leaf("a"), "a", "row", "b", "s1");

test("split replaces the target leaf and keeps visual order", () => {
  const l = splitPane(two(), "b", "col", "c", "s2");
  expect(paneIds(l)).toEqual(["a", "b", "c"]);
  expect(l.type === "split" && l.b.type === "split" && l.b.dir).toBe("col");
});

test("splitting an unknown pane changes nothing", () => {
  const l = two();
  expect(paneIds(splitPane(l, "zzz", "row", "c", "s2"))).toEqual(["a", "b"]);
});

test("removing a pane promotes its sibling", () => {
  expect(removePane(two(), "a")).toEqual(leaf("b"));
  const three = splitPane(two(), "b", "col", "c", "s2");
  const r = removePane(three, "b")!;
  expect(paneIds(r)).toEqual(["a", "c"]);
});

test("removing the last pane returns null; removing an unknown pane is a no-op", () => {
  expect(removePane(leaf("a"), "a")).toBeNull();
  const l = two();
  expect(removePane(l, "zzz")).toBe(l);
});

test("ratio is clamped and only touches the named split", () => {
  const l = splitPane(two(), "b", "col", "c", "s2");
  const hi = setRatio(l, "s1", 5);
  expect(hi.type === "split" && hi.ratio).toBe(MAX_RATIO);
  const lo = setRatio(l, "s2", -1);
  expect(lo.type === "split" && lo.ratio).toBe(0.5);
  expect(lo.type === "split" && lo.b.type === "split" && lo.b.ratio).toBe(MIN_RATIO);
});

test("cyclePane wraps in both directions", () => {
  const l = splitPane(two(), "b", "col", "c", "s2");
  expect(cyclePane(l, "a", 1)).toBe("b");
  expect(cyclePane(l, "c", 1)).toBe("a");
  expect(cyclePane(l, "a", -1)).toBe("c");
  expect(cyclePane(l, "missing", 1)).toBe("a");
});

test("hasPane", () => {
  expect(hasPane(two(), "b")).toBe(true);
  expect(hasPane(two(), "x")).toBe(false);
});

import { computeGeometry } from "./layout";

test("geometry tiles the container without overlap", () => {
  const l = splitPane(two(), "b", "col", "c", "s2");
  const g = computeGeometry(l);
  expect(g.panes.a).toEqual({ x: 0, y: 0, w: 0.5, h: 1 });
  expect(g.panes.b).toEqual({ x: 0.5, y: 0, w: 0.5, h: 0.5 });
  expect(g.panes.c).toEqual({ x: 0.5, y: 0.5, w: 0.5, h: 0.5 });
  const area = Object.values(g.panes).reduce((s, r) => s + r.w * r.h, 0);
  expect(area).toBeCloseTo(1);
  expect(g.splits.map((s) => s.id)).toEqual(["s1", "s2"]);
});

test("splitting keeps existing panes' ids stable (no remount key changes)", () => {
  const before = Object.keys(computeGeometry(two()).panes);
  const after = Object.keys(computeGeometry(splitPane(two(), "a", "col", "z", "s9")).panes);
  for (const id of before) expect(after).toContain(id);
});

test("splitting before puts the new pane left/above and leaves the old one in place", () => {
  const row = splitPane(leaf("a"), "a", "row", "n", "s1", "before");
  expect(paneIds(row)).toEqual(["n", "a"]);
  const g = computeGeometry(row);
  expect(g.panes.n).toEqual({ x: 0, y: 0, w: 0.5, h: 1 });
  expect(g.panes.a).toEqual({ x: 0.5, y: 0, w: 0.5, h: 1 });
  const col = splitPane(leaf("a"), "a", "col", "n", "s1", "before");
  expect(computeGeometry(col).panes.n.y).toBe(0);
  expect(computeGeometry(col).panes.a.y).toBe(0.5);
});

test("default placement is unchanged (after)", () => {
  expect(paneIds(splitPane(leaf("a"), "a", "row", "n", "s1"))).toEqual(["a", "n"]);
});
