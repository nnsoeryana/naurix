import { defineTheme } from "./define";

/** Naurix brand on Cloud White: the same purple/teal identity, darkened where text and icons need contrast. */
export const naurixLight = defineTheme({
  id: "naurix-light", name: "Naurix Light", kind: "light", credit: "Naurix brand palette", gradientButtons: true,
  p: { bg: "#f4f7ff", fg: "#0b1020", surface: "#ffffff", raised: "#e8edfb", line: "#d3daf0", muted: "#475073", faint: "#68729a", accent: "#6a5ae0", accent2: "#0f9d8f", accentForeground: "#ffffff", success: "#0f8a6a", warning: "#9a6200", danger: "#c42b4b", info: "#2a6fd6" },
  term: { cursor: "#6a5ae0", ansi: { black: "#0b1020", red: "#c42b4b", green: "#0f7a4e", yellow: "#8a5a00", blue: "#4b3fcf", magenta: "#9333b8", cyan: "#0b7f75", white: "#5b6490", brightBlack: "#475073", brightRed: "#d9364f", brightGreen: "#168a5a", brightYellow: "#9a6a00", brightBlue: "#5a4de0", brightMagenta: "#a24bc8", brightCyan: "#0f9d8f", brightWhite: "#7a84a8" } },
});
