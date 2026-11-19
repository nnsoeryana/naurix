import { describe, expect, it, beforeEach } from "vitest";
import { contrast, ensureContrast, isHex } from "./color";
import { readability } from "./define";
import { THEMES, getTheme, themeOrDefault } from "./registry";
import { cacheAppearance, cssVariables, readCachedAppearance, terminalTheme } from "./apply";
import { kindFor, resolveAppearance } from "./resolve";
import { DEFAULT_APPEARANCE } from "./types";
import { exportThemeJson } from "./exportTheme";

const ids = ["naurix-dark", "naurix-light", "tokyo-night", "dracula", "nord", "catppuccin-mocha", "catppuccin-latte", "github-light", "solarized-light"];

describe("registry", () => {
  it("has the nine built-in themes with unique ids", () => {
    expect(THEMES.map((t) => t.id)).toEqual(ids);
    expect(new Set(THEMES.map((t) => t.id)).size).toBe(ids.length);
  });
  it("classifies kinds", () => {
    expect(THEMES.filter((t) => t.kind === "light").map((t) => t.id)).toEqual(["naurix-light", "catppuccin-latte", "github-light", "solarized-light"]);
  });
  it("falls back for unknown ids and wrong kinds", () => {
    expect(themeOrDefault("nope", "dark").id).toBe("naurix-dark");
    expect(themeOrDefault("nope", "light").id).toBe("naurix-light");
    expect(themeOrDefault("dracula", "light").id).toBe("naurix-light");
    expect(getTheme("nord")?.name).toBe("Nord");
  });
});

describe("every theme is complete and readable", () => {
  for (const t of THEMES) {
    it(t.id, () => {
      const k = t.tokens;
      for (const v of [k.background, k.foreground, k.surface, k.surfaceSecondary, k.border, k.muted, k.faint, k.accent, k.accentForeground, k.sidebarBackground, k.dialogBackground, k.popoverBackground, k.focusRing, k.success, k.warning, k.danger, k.info, k.terminal.background, k.terminal.foreground, k.terminal.cursor, ...Object.values(k.terminal.ansi)]) expect(isHex(v), `${t.id}: ${v}`).toBe(true);
      expect(Object.keys(k.terminal.ansi)).toHaveLength(16);
      const r = readability(t);
      expect(r.fg).toBeGreaterThanOrEqual(7);
      expect(r.muted).toBeGreaterThanOrEqual(4.5);
      expect(r.onAccent).toBeGreaterThanOrEqual(4.5);
      expect(r.accent).toBeGreaterThanOrEqual(3);
      expect(contrast(k.terminal.foreground, k.terminal.background)).toBeGreaterThanOrEqual(4.5);
      for (const c of [k.success, k.warning, k.danger, k.info]) expect(contrast(c, k.background)).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("appearance mode", () => {
  it("picks the kind", () => {
    expect(kindFor("dark", false)).toBe("dark");
    expect(kindFor("light", true)).toBe("light");
    expect(kindFor("system", true)).toBe("dark");
    expect(kindFor("system", false)).toBe("light");
  });
  it("follows the system with the per-kind theme", () => {
    const a = { ...DEFAULT_APPEARANCE, mode: "system" as const, darkTheme: "tokyo-night", lightTheme: "github-light" };
    expect(resolveAppearance(a, true).id).toBe("tokyo-night");
    expect(resolveAppearance(a, false).id).toBe("github-light");
  });
  it("never throws on bad stored values", () => {
    const r = resolveAppearance({ mode: "dark", darkTheme: "gone", lightTheme: "", accent: "banana" }, true);
    expect(r.id).toBe("naurix-dark");
    expect(r.accentAdjusted).toBe(false);
  });
});

describe("custom accent", () => {
  it("applies to accent, focus ring, selection and terminal cursor, not to ANSI colors", () => {
    const base = resolveAppearance(DEFAULT_APPEARANCE, true);
    const r = resolveAppearance({ ...DEFAULT_APPEARANCE, accent: "#ff8800" }, true);
    expect(r.accent).toBe("#ff8800");
    expect(r.tokens.focusRing).toBe("#ff8800");
    expect(r.tokens.terminal.cursor).toBe("#ff8800");
    expect(r.tokens.terminal.ansi).toEqual(base.tokens.terminal.ansi);
    expect(r.gradientButtons).toBe(false);
  });
  it("keeps unreadable accents readable", () => {
    const dark = resolveAppearance({ ...DEFAULT_APPEARANCE, accent: "#0a0a14" }, true);
    expect(dark.accentAdjusted).toBe(true);
    expect(contrast(dark.accent, dark.tokens.background)).toBeGreaterThanOrEqual(3);
    const light = resolveAppearance({ ...DEFAULT_APPEARANCE, mode: "light", accent: "#fffff0" }, false);
    expect(light.accentAdjusted).toBe(true);
    expect(contrast(light.accent, light.tokens.background)).toBeGreaterThanOrEqual(3);
    expect(contrast(light.tokens.accentForeground, light.accent)).toBeGreaterThanOrEqual(4.5);
  });
  it("ensureContrast leaves passing colors alone", () => {
    expect(ensureContrast("#ffffff", "#000000", 4.5)).toBe("#ffffff");
  });
});

describe("applying", () => {
  it("emits every CSS variable the stylesheet reads", () => {
    const v = cssVariables(resolveAppearance(DEFAULT_APPEARANCE, true));
    for (const n of ["--color-midnight", "--color-cloud", "--color-surface", "--color-raised", "--color-line", "--color-muted", "--color-faint", "--color-purple", "--color-on-accent", "--color-teal", "--color-sidebar", "--color-dialog", "--color-popover", "--color-overlay", "--color-ring", "--color-selection", "--color-success", "--color-warning", "--color-danger", "--color-info", "--color-terminal", "--btn-accent"]) expect(v[n], n).toBeTruthy();
  });
  it("maps to an xterm theme without recreating anything", () => {
    const t = terminalTheme(resolveAppearance({ ...DEFAULT_APPEARANCE, darkTheme: "dracula" }, true));
    expect(t.background).toBe("#282a36");
    expect(t.cursorAccent).toBe(t.background);
    expect(t.red).toBe("#ff5555");
    expect(t.brightWhite).toBe("#ffffff");
  });
  it("exports portable JSON", () => {
    const j = JSON.parse(exportThemeJson(resolveAppearance(DEFAULT_APPEARANCE, true)));
    expect(j.naurixTheme).toBe(1);
    expect(j.id).toBe("custom-naurix-dark");
    expect(j.tokens.terminal.ansi.red).toBeTruthy();
  });
});

describe("launch cache", () => {
  beforeEach(() => {
    const m = new Map<string, string>();
    (globalThis as any).localStorage = { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
  });
  it("round-trips and tolerates garbage", () => {
    expect(readCachedAppearance()).toBeNull();
    cacheAppearance({ id: "nord", accent: "#112233" });
    expect(readCachedAppearance()).toEqual({ id: "nord", accent: "#112233" });
    localStorage.setItem("naurix.appearance.v1", "{not json");
    expect(readCachedAppearance()).toBeNull();
  });
  it("survives blocked storage", () => {
    (globalThis as any).localStorage = { getItem: () => { throw new Error("x"); }, setItem: () => { throw new Error("x"); } };
    expect(() => cacheAppearance({ id: "a", accent: null })).not.toThrow();
    expect(readCachedAppearance()).toBeNull();
  });
});
