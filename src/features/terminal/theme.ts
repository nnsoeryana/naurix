export const DEFAULT_FONT_SIZE = 13;

/**
 * Prompts such as Powerlevel10k draw icons and separators from the Private Use Area, which only
 * Nerd Fonts contain. The browser falls back per character, so a user-chosen font can come first
 * and these cover the icons. The last entries keep plain text working without any Nerd Font.
 */
export const NERD_FALLBACKS = [
  '"MesloLGS NF"',
  '"MesloLGS Nerd Font"',
  '"JetBrainsMono Nerd Font"',
  '"JetBrainsMono NF"',
  '"FiraCode Nerd Font"',
  '"CaskaydiaCove Nerd Font"',
  '"Hack Nerd Font"',
  '"Symbols Nerd Font Mono"',
  '"Symbols Nerd Font"',
  "Menlo",
  "Monaco",
  '"Courier New"',
  "monospace",
];

export function fontStack(userFamily: string | null | undefined): string {
  return [...(userFamily?.trim() ? [userFamily.trim()] : []), ...NERD_FALLBACKS].join(", ");
}
