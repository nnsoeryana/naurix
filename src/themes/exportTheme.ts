import type { ResolvedTheme } from "./resolve";

/** The current theme as portable JSON (the format future "import theme" will read). */
export function exportThemeJson(t: ResolvedTheme): string {
  return JSON.stringify({ naurixTheme: 1, id: `custom-${t.id}`, name: `${t.name} (copy)`, kind: t.kind, tokens: t.tokens }, null, 2);
}
