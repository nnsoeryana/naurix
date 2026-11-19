import { catppuccinLatte } from "./catppuccin-latte";
import { catppuccinMocha } from "./catppuccin-mocha";
import { dracula } from "./dracula";
import { githubLight } from "./github-light";
import { naurixDark } from "./naurix-dark";
import { naurixLight } from "./naurix-light";
import { nord } from "./nord";
import { solarizedLight } from "./solarized-light";
import { tokyoNight } from "./tokyo-night";
import type { ThemeDefinition, ThemeKind } from "./types";

/** Built-in themes, in display order. Custom (imported) themes will be added here at runtime later. */
export const THEMES: readonly ThemeDefinition[] = [naurixDark, naurixLight, tokyoNight, dracula, nord, catppuccinMocha, catppuccinLatte, githubLight, solarizedLight];

const byId = new Map(THEMES.map((t) => [t.id, t]));

export const DEFAULT_THEME: Record<ThemeKind, string> = { dark: "naurix-dark", light: "naurix-light" };

export const getTheme = (id: string): ThemeDefinition | undefined => byId.get(id);

/** The theme for `id`, falling back safely when it is unknown or of the wrong kind. */
export function themeOrDefault(id: string, kind: ThemeKind): ThemeDefinition {
  const t = byId.get(id);
  return t && t.kind === kind ? t : byId.get(DEFAULT_THEME[kind])!;
}

export const themesOfKind = (kind: ThemeKind) => THEMES.filter((t) => t.kind === kind);
