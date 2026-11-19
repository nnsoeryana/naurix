import { contrast, ensureContrast, mix, readableOn, withAlpha } from "./color";
import type { AnsiPalette, ThemeDefinition, ThemeKind } from "./types";

export type Palette = {
  bg: string;
  fg: string;
  surface: string;
  raised: string;
  line: string;
  accent: string;
  accent2: string;
  success: string;
  warning: string;
  danger: string;
  info: string;
  /** Optional explicit secondary/tertiary text; derived from fg/bg when omitted. */
  muted?: string;
  faint?: string;
  accentForeground?: string;
};

export type TerminalPalette = { bg?: string; fg?: string; cursor?: string; selection?: string; minContrast?: number; ansi: AnsiPalette };

/**
 * Build a full token set from a compact palette. Derived tokens are computed once here (never at
 * render time). Text-bearing tokens are nudged to keep readable contrast on the theme's own surfaces.
 */
export function defineTheme(o: { id: string; name: string; kind: ThemeKind; credit: string; gradientButtons?: boolean; p: Palette; term: TerminalPalette }): ThemeDefinition {
  const { p, term } = o;
  const muted = p.muted ?? ensureContrast(mix(p.fg, p.bg, 0.3), p.surface, 4.5);
  const faint = p.faint ?? ensureContrast(mix(p.fg, p.bg, 0.5), p.surface, 3);
  const accent = ensureContrast(p.accent, p.bg, 3);
  const dark = o.kind === "dark";
  const tbg = term.bg ?? p.bg;
  return {
    id: o.id,
    name: o.name,
    kind: o.kind,
    credit: o.credit,
    gradientButtons: o.gradientButtons,
    tokens: {
      background: p.bg,
      foreground: p.fg,
      surface: p.surface,
      surfaceSecondary: p.raised,
      border: p.line,
      muted,
      faint,
      accent,
      accentForeground: p.accentForeground ?? readableOn(accent),
      accentSecondary: ensureContrast(p.accent2, p.bg, 3),
      sidebarBackground: p.surface,
      dialogBackground: p.surface,
      popoverBackground: p.raised,
      overlayBackdrop: dark ? "rgba(0, 0, 0, 0.5)" : "rgba(15, 23, 42, 0.32)",
      focusRing: accent,
      selection: withAlpha(accent, dark ? 0.35 : 0.25),
      success: ensureContrast(p.success, p.bg, 4.5),
      warning: ensureContrast(p.warning, p.bg, 4.5),
      danger: ensureContrast(p.danger, p.bg, 4.5),
      info: ensureContrast(p.info, p.bg, 4.5),
      terminal: {
        background: tbg,
        foreground: term.fg ?? p.fg,
        cursor: term.cursor ?? accent,
        selection: term.selection ?? withAlpha(accent, dark ? 0.35 : 0.25),
        minContrast: term.minContrast ?? (dark ? 1 : 4.5),
        ansi: term.ansi,
      },
    },
  };
}

/** Used by tests and the registry: does this theme's text meet its own minimums? */
export function readability(t: ThemeDefinition) {
  const k = t.tokens;
  return { fg: contrast(k.foreground, k.background), muted: contrast(k.muted, k.surface), faint: contrast(k.faint, k.surface), accent: contrast(k.accent, k.background), onAccent: contrast(k.accentForeground, k.accent) };
}
