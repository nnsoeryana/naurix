import { defineTheme } from "./define";

/** Catppuccin Latte (MIT), catppuccin.com. */
export const catppuccinLatte = defineTheme({
  id: "catppuccin-latte", name: "Catppuccin Latte", kind: "light", credit: "Catppuccin, MIT (github.com/catppuccin/catppuccin)",
  p: { bg: "#eff1f5", fg: "#4c4f69", surface: "#e6e9ef", raised: "#ccd0da", line: "#bcc0cc", accent: "#8839ef", accent2: "#179299", success: "#40a02b", warning: "#df8e1d", danger: "#d20f39", info: "#1e66f5" },
  term: { cursor: "#dc8a78", selection: "#acb0be", ansi: { black: "#5c5f77", red: "#d20f39", green: "#40a02b", yellow: "#df8e1d", blue: "#1e66f5", magenta: "#ea76cb", cyan: "#179299", white: "#acb0be", brightBlack: "#6c6f85", brightRed: "#d20f39", brightGreen: "#40a02b", brightYellow: "#df8e1d", brightBlue: "#1e66f5", brightMagenta: "#ea76cb", brightCyan: "#179299", brightWhite: "#bcc0cc" } },
});
