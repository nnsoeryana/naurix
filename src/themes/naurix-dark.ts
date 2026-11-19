import { defineTheme } from "./define";

/** Naurix brand: Aurora Purple / Aurora Teal on Midnight (assets/brand). */
export const naurixDark = defineTheme({
  id: "naurix-dark", name: "Naurix Dark", kind: "dark", credit: "Naurix brand palette", gradientButtons: true,
  p: { bg: "#0b1020", fg: "#f4f7ff", surface: "#0f1530", raised: "#161d3d", line: "#232b52", muted: "#a3accb", faint: "#7a84a8", accent: "#8b7cff", accent2: "#39d6c5", accentForeground: "#0b1020", success: "#39d6c5", warning: "#fcd34d", danger: "#ff7a90", info: "#7cb7ff" },
  term: { cursor: "#8b7cff", selection: "rgba(139, 124, 255, 0.35)", ansi: { black: "#1a2036", red: "#ff6b81", green: "#4ade80", yellow: "#facc15", blue: "#8b7cff", magenta: "#c084fc", cyan: "#39d6c5", white: "#e6eaf5", brightBlack: "#5b6490", brightRed: "#ff8da0", brightGreen: "#86efac", brightYellow: "#fde68a", brightBlue: "#a99cff", brightMagenta: "#d8b4fe", brightCyan: "#7cefe0", brightWhite: "#ffffff" } },
});
