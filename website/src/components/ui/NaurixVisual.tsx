import { useId } from "react";

/**
 * Naurix: an original abstract aurora ribbon (SVG + CSS, transform/opacity only).
 * A brand visual. It is NOT connected to any desktop agent state.
 * Static render is the finished design; motion is progressive enhancement.
 */
export function NaurixVisual({ className = "" }: { className?: string }) {
  const id = useId();
  const main = `${id}-main`;
  const fold = `${id}-fold`;
  const glow = `${id}-glow`;
  const shine = `${id}-shine`;
  const ribbon =
    "M60 430 C130 440 165 200 290 135 C395 82 455 235 545 165 C520 275 440 335 360 285 C290 240 250 335 205 405 C165 465 105 470 60 430 Z";
  const underside = "M60 430 C130 440 165 250 250 190 C330 135 380 255 440 300 C400 330 380 310 360 285 C290 240 250 335 205 405 C165 465 105 470 60 430 Z";
  const center = "M70 428 C140 430 180 215 295 152 C395 100 450 235 535 172";

  return (
    <svg viewBox="0 0 600 560" role="img" aria-label="Naurix, an abstract Aurora-inspired ribbon of cyan, blue and violet light" className={className}>
      <defs>
        <linearGradient id={main} gradientUnits="userSpaceOnUse" x1="60" y1="440" x2="545" y2="150">
          <stop offset="0" stopColor="#9B75FF" />
          <stop offset="0.5" stopColor="#568BFF" />
          <stop offset="1" stopColor="#42D9E8" />
        </linearGradient>
        <linearGradient id={fold} gradientUnits="userSpaceOnUse" x1="80" y1="300" x2="440" y2="440">
          <stop offset="0" stopColor="#5B3FD8" />
          <stop offset="0.6" stopColor="#3F63E6" />
          <stop offset="1" stopColor="#2FC9DD" />
        </linearGradient>
        <radialGradient id={glow}>
          <stop offset="0" stopColor="#568BFF" stopOpacity="0.55" />
          <stop offset="0.55" stopColor="#9B75FF" stopOpacity="0.18" />
          <stop offset="1" stopColor="#9B75FF" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={shine} gradientUnits="userSpaceOnUse" x1="60" y1="430" x2="545" y2="165">
          <stop offset="0" stopColor="#fff" stopOpacity="0" />
          <stop offset="0.5" stopColor="#E6F8FF" stopOpacity="0.95" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
      </defs>

      <ellipse className="naurix-breathe" cx="300" cy="290" rx="290" ry="250" fill={`url(#${glow})`} />

      <g className="naurix-float">
        {/* far ribbons: softly echo the main form */}
        <g className="naurix-sway-2" opacity="0.4">
          <path d={ribbon} transform="translate(-26 38) scale(1.02)" fill="none" stroke={`url(#${main})`} strokeWidth="2" />
        </g>
        <g className="naurix-sway-1" opacity="0.28">
          <path d={ribbon} transform="translate(30 -30) scale(0.98)" fill="none" stroke={`url(#${main})`} strokeWidth="1.5" />
        </g>

        <path d={ribbon} fill={`url(#${main})`} />
        <path d={underside} fill={`url(#${fold})`} />
        <path d="M60 430 C130 440 165 250 250 190 C330 135 380 255 440 300" fill="none" stroke="#E6DDFF" strokeOpacity="0.7" strokeWidth="2.5" strokeLinecap="round" />

        {/* light flowing through the shape */}
        <path className="naurix-flow" d={center} fill="none" stroke={`url(#${shine})`} strokeWidth="5" strokeLinecap="round" opacity="0.8" />
      </g>
    </svg>
  );
}
