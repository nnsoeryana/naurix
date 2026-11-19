import { defineTheme } from "./define";

/** Solarized Light (MIT), Ethan Schoonover. UI text uses base02 and the terminal base01 (canonical base00 is only ~4:1 on base3, below the 4.5:1 minimum Naurix enforces). */
export const solarizedLight = defineTheme({
  id: "solarized-light", name: "Solarized Light", kind: "light", credit: "Solarized, MIT (ethanschoonover.com/solarized)",
  p: { bg: "#fdf6e3", fg: "#073642", surface: "#eee8d5", raised: "#e4ddc8", line: "#d3cbb3", accent: "#268bd2", accent2: "#2aa198", success: "#859900", warning: "#b58900", danger: "#dc322f", info: "#268bd2" },
  term: { fg: "#586e75", cursor: "#586e75", selection: "#eee8d5", ansi: { black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5", brightBlack: "#002b36", brightRed: "#cb4b16", brightGreen: "#586e75", brightYellow: "#657b83", brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3" } },
});
