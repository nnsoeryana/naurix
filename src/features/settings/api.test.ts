import { expect, test } from "vitest";
import { DEFAULT_SETTINGS, mergeSettings } from "./api";

test("a partial patch only changes what it names", () => {
  const s = mergeSettings(DEFAULT_SETTINGS, { terminal: { renderer: "webgl", scrollback: 20000 } });
  expect(s.terminal.renderer).toBe("webgl");
  expect(s.terminal.scrollback).toBe(20000);
  expect(s.terminal.fontSize).toBe(DEFAULT_SETTINGS.terminal.fontSize);
  expect(s.general).toEqual(DEFAULT_SETTINGS.general);
  expect(DEFAULT_SETTINGS.terminal.renderer).toBe("dom");
});
