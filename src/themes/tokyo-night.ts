import { defineTheme } from "./define";

/** Tokyo Night (MIT) by enkia / folke. Colors from the published palette. */
export const tokyoNight = defineTheme({
  id: "tokyo-night", name: "Tokyo Night", kind: "dark", credit: "Tokyo Night, MIT (enkia/tokyo-night-vscode-theme, folke/tokyonight.nvim)",
  p: { bg: "#1a1b26", fg: "#c0caf5", surface: "#16161e", raised: "#24283b", line: "#292e42", accent: "#7aa2f7", accent2: "#7dcfff", success: "#9ece6a", warning: "#e0af68", danger: "#f7768e", info: "#7dcfff" },
  term: { cursor: "#c0caf5", selection: "#283457", ansi: { black: "#15161e", red: "#f7768e", green: "#9ece6a", yellow: "#e0af68", blue: "#7aa2f7", magenta: "#bb9af7", cyan: "#7dcfff", white: "#a9b1d6", brightBlack: "#414868", brightRed: "#f7768e", brightGreen: "#9ece6a", brightYellow: "#e0af68", brightBlue: "#7aa2f7", brightMagenta: "#bb9af7", brightCyan: "#7dcfff", brightWhite: "#c0caf5" } },
});
