import { contrast, ensureContrast, isHex, readableOn, withAlpha } from "./color";
import { themeOrDefault } from "./registry";
import type { Appearance, ThemeDefinition, ThemeKind, ThemeTokens } from "./types";

export type ResolvedTheme = {
  id: string;
  name: string;
  kind: ThemeKind;
  gradientButtons: boolean;
  tokens: ThemeTokens;
  /** The user's custom accent was changed to keep it readable on this theme. */
  accentAdjusted: boolean;
  /** The accent actually in use. */
  accent: string;
};

export const kindFor = (mode: Appearance["mode"], systemDark: boolean): ThemeKind => (mode === "system" ? (systemDark ? "dark" : "light") : mode);

/**
 * Apply a custom accent without letting it hurt readability: it must stay at least 3:1 against the
 * theme background (WCAG non-text contrast), and its label color is chosen automatically.
 */
export function withAccent(base: ThemeTokens, accent: string): { tokens: ThemeTokens; adjusted: boolean; accent: string } {
  const safe = ensureContrast(accent, base.background, 3);
  const dark = base.overlayBackdrop.startsWith("rgba(0");
  return {
    adjusted: safe.toLowerCase() !== accent.toLowerCase(),
    accent: safe,
    tokens: {
      ...base,
      accent: safe,
      accentForeground: readableOn(safe),
      focusRing: safe,
      selection: withAlpha(safe, dark ? 0.35 : 0.25),
      terminal: { ...base.terminal, cursor: safe, selection: withAlpha(safe, dark ? 0.35 : 0.25) },
    },
  };
}

/** Pure: preferences + the OS appearance -> the theme to show. Unknown ids and bad colors fall back safely. */
export function resolveAppearance(a: Appearance, systemDark: boolean): ResolvedTheme {
  const kind = kindFor(a.mode, systemDark);
  const def: ThemeDefinition = themeOrDefault(kind === "dark" ? a.darkTheme : a.lightTheme, kind);
  const custom = isHex(a.accent) ? withAccent(def.tokens, a.accent) : null;
  return { id: def.id, name: def.name, kind: def.kind, gradientButtons: !!def.gradientButtons && !custom, tokens: custom?.tokens ?? def.tokens, accentAdjusted: custom?.adjusted ?? false, accent: custom?.accent ?? def.tokens.accent };
}

export { contrast };
