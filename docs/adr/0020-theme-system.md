# ADR 0020 — Theme system
**Status:** Accepted

- **Tokens as CSS variables on `<html>`, consumed by Tailwind utilities.** Tailwind v4 `@theme` already emitted variables; components keep their class names, so a theme change touches zero components. Rejected: a React context carrying colors (re-renders everything on every switch) and per-theme stylesheets (duplicate CSS, flash).
- **One derived state** (`store.theme`) from persisted preferences and the OS appearance. No second copy to drift.
- **Palettes are compact; derivations are computed once** in `defineTheme` and contrast-checked by tests, so a new theme cannot ship unreadable.
- **xterm updated in place** via `options.theme`, never recreated, so PTY, scrollback and cursor survive.
- **Persistence reuses `app.settings`** with a strict-write/tolerant-read `appearance` group; a `localStorage` cache avoids a launch flash and is optional.
- **Two slots (dark and light) plus a mode**, so "Follow system" picks the user's chosen theme for each OS appearance.
- **Legacy variable names kept** (`--color-midnight` = background, etc.) to avoid a risky mass rename; documented in THEME_SYSTEM.md.
- **Third-party palettes** (Tokyo Night, Dracula, Nord, Catppuccin, GitHub Primer, Solarized; all MIT) are mapped, not copied; each file credits its source. Solarized Light uses darker text than the canonical base00 because the original is below 4.5:1.
- **Out of scope:** import/editor UI, marketplace, sync (the JSON export and opaque ids leave room).
