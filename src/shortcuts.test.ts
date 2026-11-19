import { expect, test } from "vitest";
import { matchShortcut, SHORTCUTS, tabIndexShortcut } from "./shortcuts";

const key = (code: string, o: Partial<{ metaKey: boolean; shiftKey: boolean; altKey: boolean; ctrlKey: boolean }> = {}) =>
  ({ code, metaKey: true, shiftKey: false, altKey: false, ctrlKey: false, ...o });

test("matches plain and shifted variants distinctly", () => {
  expect(matchShortcut(key("KeyD"))).toBe("splitRight");
  expect(matchShortcut(key("KeyD", { shiftKey: true }))).toBe("splitDown");
  expect(matchShortcut(key("BracketRight", { shiftKey: true }))).toBe("nextTab");
  expect(matchShortcut(key("BracketRight"))).toBe("nextPane");
});

test("requires Cmd and ignores Ctrl/Alt combos", () => {
  expect(matchShortcut(key("KeyT", { metaKey: false }))).toBeNull();
  expect(matchShortcut(key("KeyT", { ctrlKey: true }))).toBeNull();
  expect(matchShortcut(key("KeyT", { altKey: true }))).toBeNull();
  expect(matchShortcut(key("KeyC"))).toBeNull(); // copy stays native
});

test("no two shortcuts collide", () => {
  const seen = new Set<string>();
  for (const s of Object.values(SHORTCUTS)) {
    const k = `${s.code}:${!!s.shift}`;
    expect(seen.has(k)).toBe(false);
    seen.add(k);
  }
});

test("Cmd+digit selects tabs", () => {
  expect(tabIndexShortcut(key("Digit1"))).toBe(0);
  expect(tabIndexShortcut(key("Digit9"))).toBe(8);
  expect(tabIndexShortcut(key("Digit0"))).toBeNull();
  expect(tabIndexShortcut(key("Digit1", { shiftKey: true }))).toBeNull();
});
