import { defineTheme } from "./define";

/** Nord (MIT), nordtheme.com. nord0-nord15. */
export const nord = defineTheme({
  id: "nord", name: "Nord", kind: "dark", credit: "Nord, MIT (nordtheme.com)",
  p: { bg: "#2e3440", fg: "#eceff4", surface: "#2b303b", raised: "#3b4252", line: "#434c5e", accent: "#88c0d0", accent2: "#8fbcbb", success: "#a3be8c", warning: "#ebcb8b", danger: "#bf616a", info: "#81a1c1" },
  term: { fg: "#d8dee9", cursor: "#d8dee9", selection: "#434c5e", ansi: { black: "#3b4252", red: "#bf616a", green: "#a3be8c", yellow: "#ebcb8b", blue: "#81a1c1", magenta: "#b48ead", cyan: "#88c0d0", white: "#e5e9f0", brightBlack: "#4c566a", brightRed: "#bf616a", brightGreen: "#a3be8c", brightYellow: "#ebcb8b", brightBlue: "#81a1c1", brightMagenta: "#b48ead", brightCyan: "#8fbcbb", brightWhite: "#eceff4" } },
});
