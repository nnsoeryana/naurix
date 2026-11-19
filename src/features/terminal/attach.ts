/**
 * Attaching files to a terminal (drag and drop, pasted screenshots).
 *
 * Naurix only types file *paths* into the terminal, like Terminal.app and iTerm do. It never reads the
 * file, never presses Enter, and the agent decides what to do with the path. Content comes from the OS
 * drop event or the user's own paste, nothing in the background.
 */

const MAX_PATHS = 20;
/** Control characters (including newline and ESC) in a path could submit or alter the command line. */
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f]/;

export const IS_WINDOWS = typeof navigator !== "undefined" && /Win/.test(navigator.platform || "");

/** Quote one path so a shell reads it as a single word; plain paths are left readable. */
export function quotePath(path: string, windows: boolean = IS_WINDOWS): string {
  if (windows) {
    return /^[A-Za-z0-9_\\/.:~-]+$/.test(path) ? path : `"${path}"`;
  }
  return /^[A-Za-z0-9_/.,:=@%+~-]+$/.test(path) ? path : `'${path.replace(/'/g, `'\\''`)}'`;
}

/**
 * The text to paste for dropped paths: quoted, space separated, with a trailing space so the user can
 * keep typing. Paths with control characters are skipped; at most MAX_PATHS are used. Empty if none.
 */
export function pathsToPaste(paths: string[], windows: boolean = IS_WINDOWS): string {
  const ok = paths.filter((p) => p.length > 0 && !CONTROL.test(p)).slice(0, MAX_PATHS);
  return ok.length === 0 ? "" : ok.map((p) => quotePath(p, windows)).join(" ") + " ";
}

type ClipboardLike = Pick<DataTransfer, "types" | "items" | "getData"> | null | undefined;

/** True if a paste carries an image and no text (a screenshot), which xterm would silently drop. */
export function clipboardHasOnlyImage(dt: ClipboardLike): boolean {
  if (!dt) return false;
  const hasText = Array.from(dt.types ?? []).includes("text/plain") && dt.getData("text/plain").length > 0;
  if (hasText) return false;
  return Array.from(dt.items ?? []).some((i) => i.kind === "file" && i.type.startsWith("image/"));
}

/** Agent CLIs (Claude Code, OpenCode, Gemini CLI, Codex) read the clipboard image themselves on Ctrl+V. */
export const IMAGE_PASTE_KEY = "\x16";
