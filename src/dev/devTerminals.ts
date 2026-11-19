import type { Terminal } from "@xterm/xterm";

/** Dev builds only: lets the smoke test read terminal buffers. Tree-shaken from release builds. */
const terms = new Map<string, Terminal>();
export const registerDevTerminal = (paneId: string, t: Terminal) => void terms.set(paneId, t);
export const getDevTerminal = (paneId: string) => terms.get(paneId);
export const unregisterDevTerminal = (paneId: string) => void terms.delete(paneId);

export function dumpDevTerminals(): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const [id, t] of terms) {
    const b = t.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < b.length; i++) lines.push(b.getLine(i)?.translateToString(true) ?? "");
    out[id] = lines.filter((l) => l.trim() !== "");
  }
  return out;
}
