import { defineTheme } from "./define";

/** Catppuccin Mocha (MIT), catppuccin.com. */
export const catppuccinMocha = defineTheme({
  id: "catppuccin-mocha", name: "Catppuccin Mocha", kind: "dark", credit: "Catppuccin, MIT (github.com/catppuccin/catppuccin)",
  p: { bg: "#1e1e2e", fg: "#cdd6f4", surface: "#181825", raised: "#313244", line: "#45475a", accent: "#cba6f7", accent2: "#94e2d5", success: "#a6e3a1", warning: "#f9e2af", danger: "#f38ba8", info: "#89b4fa" },
  term: { cursor: "#f5e0dc", selection: "#585b70", ansi: { black: "#45475a", red: "#f38ba8", green: "#a6e3a1", yellow: "#f9e2af", blue: "#89b4fa", magenta: "#f5c2e7", cyan: "#94e2d5", white: "#bac2de", brightBlack: "#585b70", brightRed: "#f38ba8", brightGreen: "#a6e3a1", brightYellow: "#f9e2af", brightBlue: "#89b4fa", brightMagenta: "#f5c2e7", brightCyan: "#94e2d5", brightWhite: "#a6adc8" } },
});
