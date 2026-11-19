/** Pure split-pane layout tree. No React, no Tauri: easy to test. */
export type SplitDir = "row" | "col"; // row = panes side by side, col = stacked
export type Leaf = { type: "leaf"; paneId: string };
export type Split = { type: "split"; id: string; dir: SplitDir; ratio: number; a: Layout; b: Layout };
export type Layout = Leaf | Split;

export const MIN_RATIO = 0.1;
export const MAX_RATIO = 0.9;

export const leaf = (paneId: string): Leaf => ({ type: "leaf", paneId });

/** Where the new pane goes relative to the old one: after = right/below, before = left/above. */
export type Placement = "after" | "before";

/** Replace `paneId` with a split holding the old pane and the new pane (new one first when `before`). */
export function splitPane(root: Layout, paneId: string, dir: SplitDir, newPaneId: string, splitId: string, placement: Placement = "after"): Layout {
  if (root.type === "leaf") {
    if (root.paneId !== paneId) return root;
    const fresh = leaf(newPaneId);
    return { type: "split", id: splitId, dir, ratio: 0.5, a: placement === "after" ? root : fresh, b: placement === "after" ? fresh : root };
  }
  return { ...root, a: splitPane(root.a, paneId, dir, newPaneId, splitId, placement), b: splitPane(root.b, paneId, dir, newPaneId, splitId, placement) };
}

/** Remove a pane; its sibling takes the parent's place. Returns null if it was the last pane. */
export function removePane(root: Layout, paneId: string): Layout | null {
  if (root.type === "leaf") return root.paneId === paneId ? null : root;
  const a = removePane(root.a, paneId);
  const b = removePane(root.b, paneId);
  if (a === null) return b;
  if (b === null) return a;
  return a === root.a && b === root.b ? root : { ...root, a, b };
}

/** Pane ids in visual order (left-to-right / top-to-bottom). */
export function paneIds(root: Layout): string[] {
  return root.type === "leaf" ? [root.paneId] : [...paneIds(root.a), ...paneIds(root.b)];
}

export function hasPane(root: Layout, paneId: string): boolean {
  return paneIds(root).includes(paneId);
}

export function setRatio(root: Layout, splitId: string, ratio: number): Layout {
  if (root.type === "leaf") return root;
  const r = Math.min(MAX_RATIO, Math.max(MIN_RATIO, ratio));
  if (root.id === splitId) return { ...root, ratio: r };
  return { ...root, a: setRatio(root.a, splitId, ratio), b: setRatio(root.b, splitId, ratio) };
}

/** Next/previous pane in visual order, wrapping around. */
export function cyclePane(root: Layout, paneId: string, delta: 1 | -1): string {
  const ids = paneIds(root);
  const i = ids.indexOf(paneId);
  if (i < 0) return ids[0];
  return ids[(i + delta + ids.length) % ids.length];
}

export type Rect = { x: number; y: number; w: number; h: number }; // fractions of the container, 0..1
export type SplitGeometry = { id: string; dir: SplitDir; rect: Rect; ratio: number };

/**
 * Geometry for rendering panes as a flat, stable list (keyed by pane id) instead of a nested
 * tree. A nested tree would remount a terminal whenever its pane is split, killing the shell.
 */
export function computeGeometry(root: Layout): { panes: Record<string, Rect>; splits: SplitGeometry[] } {
  const panes: Record<string, Rect> = {};
  const splits: SplitGeometry[] = [];
  const walk = (n: Layout, r: Rect) => {
    if (n.type === "leaf") {
      panes[n.paneId] = r;
      return;
    }
    splits.push({ id: n.id, dir: n.dir, rect: r, ratio: n.ratio });
    if (n.dir === "row") {
      walk(n.a, { x: r.x, y: r.y, w: r.w * n.ratio, h: r.h });
      walk(n.b, { x: r.x + r.w * n.ratio, y: r.y, w: r.w * (1 - n.ratio), h: r.h });
    } else {
      walk(n.a, { x: r.x, y: r.y, w: r.w, h: r.h * n.ratio });
      walk(n.b, { x: r.x, y: r.y + r.h * n.ratio, w: r.w, h: r.h * (1 - n.ratio) });
    }
  };
  walk(root, { x: 0, y: 0, w: 1, h: 1 });
  return { panes, splits };
}
