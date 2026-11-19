import { defineTheme } from "./define";

/** Dracula (MIT), draculatheme.com. */
export const dracula = defineTheme({
  id: "dracula", name: "Dracula", kind: "dark", credit: "Dracula Theme, MIT (draculatheme.com)",
  p: { bg: "#282a36", fg: "#f8f8f2", surface: "#21222c", raised: "#343746", line: "#44475a", accent: "#bd93f9", accent2: "#8be9fd", success: "#50fa7b", warning: "#f1fa8c", danger: "#ff5555", info: "#8be9fd" },
  term: { cursor: "#f8f8f2", selection: "#44475a", ansi: { black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c", blue: "#bd93f9", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2", brightBlack: "#6272a4", brightRed: "#ff6e6e", brightGreen: "#69ff94", brightYellow: "#ffffa5", brightBlue: "#d6acff", brightMagenta: "#ff92df", brightCyan: "#a4ffff", brightWhite: "#ffffff" } },
});
