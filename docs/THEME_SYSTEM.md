# Theme system

Naurix themes the whole app (shell, sidebar, tabs, panes, Command Palette, Settings, MCP/Skills/Git views, dialogs) **and** the xterm.js terminal from one set of semantic tokens. Decision record: [ADR 0020](adr/0020-theme-system.md).

## Architecture

```
src/themes/
  types.ts       ThemeTokens, ThemeDefinition, Appearance
  color.ts       hex helpers: contrast (WCAG), mix, ensureContrast, readableOn
  define.ts      defineTheme(): compact palette -> full tokens (derivations computed once, contrast-nudged)
  <theme>.ts     nine built-in palettes
  registry.ts    THEMES, getTheme, themeOrDefault (safe fallback)
  resolve.ts     resolveAppearance(preferences, systemIsDark) -> ResolvedTheme (pure); custom accent
  apply.ts       CSS variables, xterm theme object, launch cache
  boot.ts        paint the cached theme before React renders (no flash)
src/features/appearance/   ThemeCard, AppearanceSettings (Settings -> Appearance)
```

Flow: `settings.appearance` (persisted) + `systemDark` (OS) → `store.theme` (the **only** derived copy; replaced only when id/accent change) → `App` writes CSS variables to `<html>` once per change; each `TerminalView` assigns `term.options.theme`. Components never receive the theme as props; Tailwind utilities (`bg-surface`, `text-muted`, `text-danger`, `bg-dialog`, …) read the variables, so a switch is one batch of `setProperty` calls and no component re-renders for color.

## Tokens

| Group | Tokens (CSS variable) |
|---|---|
| Application | background `--color-midnight`, foreground `--color-cloud`, surface `--color-surface`, surfaceSecondary `--color-raised`, border `--color-line`, muted, faint, accent `--color-purple`, accentForeground `--color-on-accent`, accentSecondary `--color-teal` |
| Chrome | sidebar, dialog, popover, overlay |
| Interactive | focus ring, selection (hover/active use `surfaceSecondary`; disabled uses reduced opacity) |
| Status | success, warning, danger, info (always shown with text or an icon too) |
| Terminal | background, foreground, cursor, selection, minContrast, 16 ANSI colors |

(The legacy Aurora variable names `midnight`, `purple`, `teal`, `cloud` were kept as the variable names to avoid a risky mass rename; they now mean *background / accent / secondary accent / foreground*.)

## Adding a theme

1. Create `src/themes/my-theme.ts` with `defineTheme({ id, name, kind, credit, p: {...}, term: { ansi: {...} } })`.
2. Add it to `THEMES` in `registry.ts`.
3. `pnpm test`: the suite checks every theme for complete valid colors and contrast (foreground ≥ 7:1, muted ≥ 4.5:1, status colors ≥ 4.5:1, text on accent ≥ 4.5:1, terminal text ≥ 4.5:1).

Palettes are mapped into Naurix's schema from each project's published colors (all MIT; credits in each file). No third-party theme code is copied.

## Terminal synchronization

`term.options.theme = …` and `options.minimumContrastRatio` are assigned on the live `Terminal`. xterm repaints in place: the PTY session, scrollback, screen contents and cursor are untouched; no terminal is recreated, no shell restarts. Light themes use `minimumContrastRatio` 4.5 so programs that color text too faintly stay readable.

## Persistence

`settings.appearance { mode: dark|light|system, darkTheme, lightTheme, accent|null }` inside the existing `app.settings` JSON document (no new table; no migration because missing groups take defaults). Writes are validated strictly (id charset/length, `#rrggbb`), reads are tolerant (a bad value falls back to its default, an unknown theme id falls back in the registry). A small `localStorage` cache of the last theme id and accent lets the next launch paint correct colors before settings load; it is optional and failure-safe.

## Accent

Only the app accent (selection, focus, primary buttons, terminal cursor) changes. ANSI colors are never touched. A custom accent is nudged until it has ≥ 3:1 contrast against the theme background and the label color on it is chosen automatically; the UI says when it was adjusted.

## Custom themes (prepared, not built)

Theme ids are opaque strings in storage; `exportThemeJson` already writes the portable format (`naurixTheme: 1`). Import would validate tokens, then register the theme at runtime under a `custom-` id.

## Accessibility

Contrast is enforced by construction and by tests; keyboard focus is a 2 px ring in every theme (`:focus-visible`); selected themes show a check and the word "Selected" (not only color); mode buttons expose `role=radio`/`aria-checked`; Follow System tracks the OS through `prefers-color-scheme`; status colors always accompany text or icons.
