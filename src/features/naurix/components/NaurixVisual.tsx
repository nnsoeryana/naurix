import { useId } from "react";
import type { NaurixMode } from "../types";
import "../animations/naurix.css";


/** A wave of `n` half-periods (16 units each) starting at x0. One full period is 32 units. */
const wave = (y: number, amp: number, x0: number, n: number) => {
  let d = `M${x0} ${y} Q${x0 + 8} ${y - amp * 2} ${x0 + 16} ${y}`;
  for (let i = 1; i < n; i++) d += ` T${x0 + 16 * (i + 1)} ${y}`;
  return d;
};
const RAYS = Array.from({ length: 14 }, (_, i) => (i * 360) / 14);
const MOTES = [
  { x: -22, y: -18 }, { x: 24, y: -14 }, { x: -18, y: 22 }, { x: 20, y: 24 }, { x: 0, y: -26 }, { x: 4, y: 28 },
];

/** Original aurora ribbon in Naurix cyan/blue/violet (theme tokens). Only the active state is in the DOM. */
export function NaurixVisual({ mode, size = 44 }: { mode: NaurixMode; size?: number }) {
  const g = useId();
  const stroke = `url(#${g}-s)`;
  return (
    <svg viewBox="0 0 64 64" width={size} height={size} aria-hidden="true" focusable="false" className="nx-svg" data-mode={mode}>
      <defs>
        <linearGradient id={`${g}-s`} gradientUnits="userSpaceOnUse" x1="4" y1="0" x2="60" y2="0">
          <stop offset="0" style={{ stopColor: "var(--nx-a)" }} />
          <stop offset="0.5" style={{ stopColor: "var(--nx-b)" }} />
          <stop offset="1" style={{ stopColor: "var(--nx-c)" }} />
        </linearGradient>
        <radialGradient id={`${g}-g`}>
          <stop offset="0" style={{ stopColor: "var(--nx-c)", stopOpacity: 0.9 }} />
          <stop offset="1" style={{ stopColor: "var(--nx-c)", stopOpacity: 0 }} />
        </radialGradient>
      </defs>
      <rect width="64" height="64" rx="14" className="nx-tile" />
      <ellipse cx="32" cy="32" rx="30" ry="30" fill={`url(#${g}-g)`} className="nx-aura" />
      {(mode === "idle" || mode === "ready" || mode === "working") && <circle cx="32" cy="32" r="18" fill={`url(#${g}-g)`} className="nx-glow" />}

      {(mode === "idle" || mode === "ready") && (
        <g className="nx-float">
          <ellipse cx="32" cy="50" rx="16" ry="2.5" className="nx-shadow" />
          <path d="M10 34 Q18 18 26 34 T42 34 T58 34" fill="none" stroke={stroke} strokeWidth="11" strokeLinecap="round" opacity="0.25" />
          <path d="M10 34 Q18 18 26 34 T42 34 T58 34" fill="none" stroke={stroke} strokeWidth="6" strokeLinecap="round" />
          <path d="M12 38 Q20 24 28 38 T44 38 T58 36" fill="none" stroke={stroke} strokeWidth="2" strokeLinecap="round" opacity="0.6" />
        </g>
      )}

      {mode === "working" && (
        <g transform="rotate(-14 32 32)">
          {[
            { y: 26, a: 7, w: 2, d: 1.3 }, { y: 30, a: 9, w: 5, d: 1.9 }, { y: 34, a: 6, w: 1.5, d: 1.1 },
            { y: 37, a: 8, w: 3, d: 1.6 }, { y: 41, a: 5, w: 1.5, d: 2.2 },
          ].map((r, i) => (
            <g key={i} className="nx-bob" style={{ animationDelay: `${-i * 0.35}s`, ["--b" as string]: `${0.9 + i * 0.15}s` }}>
              <path d={wave(r.y, r.a, -64, 12)} fill="none" stroke={stroke} strokeWidth={r.w} strokeLinecap="round" className="nx-flow" style={{ ["--d" as string]: `${r.d}s` }} />
            </g>
          ))}
        </g>
      )}

      {mode === "waiting" && (
        <g>
          <g className="nx-gather">
            <path d="M35 7 C21 19 45 26 32 36 S25 51 33 58" fill="none" stroke={stroke} strokeWidth="2.6" strokeLinecap="round" opacity="0.85" />
            {MOTES.map((m, i) => (
              <circle key={i} cx="32" cy="32" r="1.4" className="nx-mote" style={{ ["--dx" as string]: `${m.x}px`, ["--dy" as string]: `${m.y}px`, animationDelay: `${i * 0.45}s` }} />
            ))}
          </g>
        </g>
      )}

      {mode === "success" && (
        <g className="nx-burst">
          {RAYS.map((a) => (
            <line key={a} x1="32" y1="26" x2="32" y2="6" stroke={stroke} strokeWidth="1.6" strokeLinecap="round" transform={`rotate(${a} 32 32)`} />
          ))}
          <circle cx="32" cy="32" r="9" className="nx-core" />
          {[[14, 16], [50, 14], [48, 50], [16, 48]].map(([x, y], i) => (
            <circle key={i} cx={x} cy={y} r="1.5" className="nx-spark" style={{ animationDelay: `${i * 0.18}s` }} />
          ))}
        </g>
      )}

      {mode === "error" && (
        <g className="nx-sway">
          <path d="M6 36 Q18 20 32 36 T58 34" fill="none" stroke={stroke} strokeWidth="6" strokeLinecap="round" opacity="0.55" />
          <path d="M6 42 Q18 30 32 42 T58 40" fill="none" strokeWidth="1.8" strokeLinecap="round" className="nx-warn" />
        </g>
      )}
    </svg>
  );
}
