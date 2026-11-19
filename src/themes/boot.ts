import { applyTheme, readCachedAppearance } from "./apply";
import { getTheme } from "./registry";
import { resolveAppearance } from "./resolve";

/**
 * Paint the last used theme before React renders, so launching in a light theme does not flash dark
 * while the saved settings load. Purely a convenience: the real settings replace it a moment later.
 */
export function paintCachedTheme(root: HTMLElement = document.documentElement): void {
  const c = readCachedAppearance();
  const def = c ? getTheme(c.id) : undefined;
  if (!c || !def) return;
  const a = { mode: def.kind, darkTheme: def.id, lightTheme: def.id, accent: c.accent };
  applyTheme(root, resolveAppearance(a, def.kind === "dark"));
}
