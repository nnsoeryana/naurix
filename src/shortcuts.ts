/** Central shortcut table (UX-001). Matching uses physical key codes so Shift+[ etc. work. */
export type ShortcutId =
  | "palette" | "newTab" | "closePane" | "splitRight" | "splitDown"
  | "nextTab" | "prevTab" | "nextPane" | "prevPane" | "addWorkspace" | "renameTab" | "launchAgent" | "settings";

export type Shortcut = { code: string; shift?: boolean; label: string };

export const SHORTCUTS: Record<ShortcutId, Shortcut> = {
  palette: { code: "KeyK", label: "⌘K" },
  newTab: { code: "KeyT", label: "⌘T" },
  closePane: { code: "KeyW", label: "⌘W" },
  splitRight: { code: "KeyD", label: "⌘D" },
  splitDown: { code: "KeyD", shift: true, label: "⇧⌘D" },
  nextTab: { code: "BracketRight", shift: true, label: "⇧⌘]" },
  prevTab: { code: "BracketLeft", shift: true, label: "⇧⌘[" },
  nextPane: { code: "BracketRight", label: "⌘]" },
  prevPane: { code: "BracketLeft", label: "⌘[" },
  addWorkspace: { code: "KeyO", label: "⌘O" },
  renameTab: { code: "KeyR", shift: true, label: "⇧⌘R" },
  launchAgent: { code: "KeyA", shift: true, label: "⇧⌘A" },
  settings: { code: "Comma", label: "⌘," },
};

type KeyLike = Pick<KeyboardEvent, "code" | "metaKey" | "shiftKey" | "altKey" | "ctrlKey">;

export function matchShortcut(e: KeyLike): ShortcutId | null {
  if (!e.metaKey || e.altKey || e.ctrlKey) return null;
  for (const [id, s] of Object.entries(SHORTCUTS) as [ShortcutId, Shortcut][]) {
    if (e.code === s.code && !!s.shift === e.shiftKey) return id;
  }
  return null;
}

/** Cmd+1..9 selects the Nth tab; returns the zero-based index or null. */
export function tabIndexShortcut(e: KeyLike): number | null {
  if (!e.metaKey || e.altKey || e.ctrlKey || e.shiftKey) return null;
  const m = /^Digit([1-9])$/.exec(e.code);
  return m ? Number(m[1]) - 1 : null;
}
