/** `/Users/me/Documents/app` -> `~/Documents/app` (macOS home folders live under /Users). */
export function tildePath(p: string): string {
  return p.replace(/^\/Users\/[^/]+(?=\/|$)/, "~");
}

/** A tab title like the brand mock-up (`~/workspace`), numbered when the same folder repeats. */
export function uniqueTitle(base: string, existing: string[]): string {
  if (!existing.includes(base)) return base;
  for (let n = 2; ; n++) {
    const t = `${base} (${n})`;
    if (!existing.includes(t)) return t;
  }
}
