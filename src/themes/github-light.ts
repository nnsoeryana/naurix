import { defineTheme } from "./define";

/** GitHub Light (MIT), primer/github-vscode-theme. */
export const githubLight = defineTheme({
  id: "github-light", name: "GitHub Light", kind: "light", credit: "GitHub Primer, MIT (github.com/primer/github-vscode-theme)",
  p: { bg: "#ffffff", fg: "#24292f", surface: "#f6f8fa", raised: "#eaeef2", line: "#d0d7de", accent: "#0969da", accent2: "#1b7c83", success: "#1a7f37", warning: "#9a6700", danger: "#cf222e", info: "#0969da" },
  term: { cursor: "#0969da", selection: "#b6d4fe", ansi: { black: "#24292f", red: "#cf222e", green: "#116329", yellow: "#4d2d00", blue: "#0969da", magenta: "#8250df", cyan: "#1b7c83", white: "#6e7781", brightBlack: "#57606a", brightRed: "#a40e26", brightGreen: "#1a7f37", brightYellow: "#633c01", brightBlue: "#218bff", brightMagenta: "#a475f9", brightCyan: "#3192aa", brightWhite: "#8c959f" } },
});
