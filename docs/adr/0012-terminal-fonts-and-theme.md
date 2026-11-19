# ADR 0012 — Terminal fonts and default theme
**Status:** Accepted

**Context:** Common prompts (Powerlevel10k with `POWERLEVEL9K_MODE=nerdfont-v3`, Starship, etc.) use Private Use Area glyphs from Nerd Fonts. Other terminals (e.g. Ghostty) bundle a symbols fallback; a WebView does not. The default Menlo font rendered those icons as empty boxes.

**Decision:**
- The xterm.js font stack is `<user font>, <known Nerd Fonts>, Menlo, Monaco, "Courier New", monospace`. Per-character fallback in the WebView means a normal text font can be combined with a Nerd Font that supplies icons. Fonts are never bundled (licensing, size); they are used only if the user has them installed.
- `terminal.fontFamily` and `terminal.fontSize` are stored in the `settings` table, validated in Rust (family restricted to letters, digits, space and `, ' " - _ .` so it cannot break out of a CSS font list; size 8–32), with corrupt stored values falling back to defaults. Editing is via the command palette until the Settings screen (M10).
- The default terminal background was first `#051625` (sampled from a reference screenshot); ADR 0013 replaces it with the brand's Midnight `#0B1020`. Other colors keep xterm.js defaults for now; a theme system is TERM-013 (P1).
- The DOM renderer draws the glyphs from the font. xterm.js's built-in custom glyph drawing (for box/powerline characters) only applies to its canvas/WebGL renderers, so it becomes relevant if the renderer is changed in M10.

**Verified:** in the real app with the maintainer's p10k prompt, the Apple logo, folder icon and powerline separators render with `MesloLGS NF`; the WebView sees both `MesloLGS NF` and `Dank Mono` as available.
