import { expect, test } from "vitest";
import { fontStack, NERD_FALLBACKS } from "./theme";

test("user font comes first, Nerd Font fallbacks follow", () => {
  const s = fontStack("'Dank Mono'");
  expect(s.startsWith("'Dank Mono', ")).toBe(true);
  expect(s).toContain('"MesloLGS NF"');
  expect(s.endsWith("monospace")).toBe(true);
});

test("default stack has fallbacks and no empty entries", () => {
  expect(fontStack(null)).toBe(NERD_FALLBACKS.join(", "));
  expect(fontStack("  ")).toBe(NERD_FALLBACKS.join(", "));
  expect(fontStack(null).split(",").every((p) => p.trim() !== "")).toBe(true);
});
