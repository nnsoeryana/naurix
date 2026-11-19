/** Small, dependency-free color helpers (hex only). Used for contrast-safe accents and derived tokens. */
export type RGB = [number, number, number];

export const isHex = (s: unknown): s is string => typeof s === "string" && /^#[0-9a-fA-F]{6}$/.test(s);

export function parseHex(hex: string): RGB {
  if (!isHex(hex)) throw new Error(`not a #rrggbb color: ${String(hex)}`);
  return [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
}

const h2 = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, "0");
export const toHex = ([r, g, b]: RGB): string => `#${h2(r)}${h2(g)}${h2(b)}`;

/** WCAG relative luminance. */
export function luminance(hex: string): number {
  const lin = parseHex(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}

/** WCAG contrast ratio, 1..21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** Linear mix: t=0 -> a, t=1 -> b. */
export function mix(a: string, b: string, t: number): string {
  const [x, y] = [parseHex(a), parseHex(b)];
  return toHex([0, 1, 2].map((i) => x[i] * (1 - t) + y[i] * t) as RGB);
}

export function withAlpha(hex: string, alpha: number): string {
  const [r, g, b] = parseHex(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Whichever of near-black / white reads better on `bg`. */
export function readableOn(bg: string, dark = "#0b1020", light = "#ffffff"): string {
  return contrast(bg, dark) >= contrast(bg, light) ? dark : light;
}

/**
 * Nudge `fg` toward black or white (whichever direction can reach it) until it has at least `min`
 * contrast against `bg`. Returns `fg` unchanged if it already passes.
 */
export function ensureContrast(fg: string, bg: string, min: number): string {
  if (contrast(fg, bg) >= min) return fg;
  const target = luminance(bg) > 0.4 ? "#000000" : "#ffffff";
  for (let i = 1; i <= 40; i++) {
    const c = mix(fg, target, i / 40);
    if (contrast(c, bg) >= min) return c;
  }
  return target;
}
