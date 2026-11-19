export type ThemeKind = "dark" | "light";

export type AnsiPalette = {
  black: string; red: string; green: string; yellow: string; blue: string; magenta: string; cyan: string; white: string;
  brightBlack: string; brightRed: string; brightGreen: string; brightYellow: string; brightBlue: string; brightMagenta: string; brightCyan: string; brightWhite: string;
};

/**
 * Semantic tokens. Components never name a palette color; they use these (through Tailwind utilities
 * that read CSS variables). All values are `#rrggbb` except the two that are intentionally translucent
 * (`overlayBackdrop`, `selection`, `terminal.selection`), which are `rgba(...)`.
 */
export type ThemeTokens = {
  // application
  background: string;
  foreground: string;
  surface: string;
  surfaceSecondary: string; // raised panels, hover and selected rows
  border: string;
  muted: string; // secondary text
  faint: string; // tertiary text, placeholders
  accent: string;
  accentForeground: string; // text on top of `accent`
  accentSecondary: string; // running state, links, success-leaning highlights
  // chrome
  sidebarBackground: string;
  dialogBackground: string;
  popoverBackground: string;
  overlayBackdrop: string;
  // interactive
  focusRing: string;
  selection: string;
  // status (also used with a text label or icon, never as the only signal)
  success: string;
  warning: string;
  danger: string;
  info: string;
  terminal: {
    background: string;
    foreground: string;
    cursor: string;
    selection: string;
    /** xterm `minimumContrastRatio`: raises the contrast of text the program colored too faintly. */
    minContrast: number;
    ansi: AnsiPalette;
  };
};

export type ThemeDefinition = {
  /** Stable id, `[a-z0-9-]`. Custom (imported) themes will use a `custom-` prefix. */
  id: string;
  name: string;
  kind: ThemeKind;
  /** Where the palette comes from, shown in docs and the About text. */
  credit: string;
  /** Solid accent buttons (default) or the Naurix accent gradient. */
  gradientButtons?: boolean;
  tokens: ThemeTokens;
};

export type AppearanceMode = "system" | "dark" | "light";

/** Persisted appearance preferences (mirrors `naurix_core::settings::Appearance`). */
export type Appearance = {
  mode: AppearanceMode;
  darkTheme: string;
  lightTheme: string;
  /** Custom accent, `#rrggbb`, or null for the theme's own. */
  accent: string | null;
};

export const DEFAULT_APPEARANCE: Appearance = { mode: "dark", darkTheme: "naurix-dark", lightTheme: "naurix-light", accent: null };
