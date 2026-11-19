import type { ResolvedTheme } from "./resolve";

/** CSS variables (consumed by Tailwind utilities in `src/index.css`) for a resolved theme. */
export function cssVariables(t: ResolvedTheme): Record<string, string> {
  const k = t.tokens;
  return {
    "--color-midnight": k.background,
    "--color-cloud": k.foreground,
    "--color-surface": k.surface,
    "--color-raised": k.surfaceSecondary,
    "--color-line": k.border,
    "--color-muted": k.muted,
    "--color-faint": k.faint,
    "--color-purple": k.accent,
    "--color-on-accent": k.accentForeground,
    "--color-teal": k.accentSecondary,
    "--color-sidebar": k.sidebarBackground,
    "--color-dialog": k.dialogBackground,
    "--color-popover": k.popoverBackground,
    "--color-overlay": k.overlayBackdrop,
    "--color-ring": k.focusRing,
    "--color-selection": k.selection,
    "--color-success": k.success,
    "--color-warning": k.warning,
    "--color-danger": k.danger,
    "--color-info": k.info,
    "--color-terminal": k.terminal.background,
    "--btn-accent": t.gradientButtons ? `linear-gradient(135deg, ${k.accent}, ${k.accentSecondary})` : `linear-gradient(${k.accent}, ${k.accent})`,
  };
}

/** Write the theme to an element (normally `<html>`). Only changes properties whose value differs. */
export function applyTheme(root: HTMLElement, t: ResolvedTheme): void {
  const vars = cssVariables(t);
  for (const [name, value] of Object.entries(vars)) {
    if (root.style.getPropertyValue(name) !== value) root.style.setProperty(name, value);
  }
  root.dataset.theme = t.id;
  root.style.colorScheme = t.kind;
}

/** The xterm.js theme object for a resolved theme (assigned to `term.options.theme`; no terminal is recreated). */
export function terminalTheme(t: ResolvedTheme) {
  const k = t.tokens.terminal;
  return { background: k.background, foreground: k.foreground, cursor: k.cursor, cursorAccent: k.background, selectionBackground: k.selection, ...k.ansi };
}

const CACHE_KEY = "naurix.appearance.v1";

/** Remember the last applied appearance so the next launch can paint the right colors before settings load. */
export function cacheAppearance(a: { id: string; accent: string | null }): void {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(a));
  } catch {
    /* private window or blocked storage: the app still works, it just may flash the default once */
  }
}

export function readCachedAppearance(): { id: string; accent: string | null } | null {
  try {
    const v = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
    return v && typeof v.id === "string" ? { id: v.id, accent: typeof v.accent === "string" ? v.accent : null } : null;
  } catch {
    return null;
  }
}
