/**
 * Aurora lighting from gradients (no canvas; curtains use a small static blur): soft glow blobs plus, at full intensity,
 * vertical "curtains" (ray stripes faded top and bottom) that sway and shimmer like northern lights.
 * Static layout looks finished; animation is transform/opacity-only and disabled under reduced motion.
 */
export function AuroraBackground({ className = "", intensity = "full" }: { className?: string; intensity?: "full" | "soft" }) {
  const o = intensity === "full" ? 1 : 0.55;
  return (
    <div aria-hidden="true" className={`pointer-events-none absolute inset-0 overflow-hidden ${className}`} style={{ opacity: o }}>
      <div
        className="aurora-a absolute -left-[10%] -top-[20%] h-[75%] w-[70%] will-change-transform"
        style={{ background: "radial-gradient(closest-side, rgba(66,217,232,0.28), transparent 72%)" }}
      />
      <div
        className="aurora-b absolute -right-[10%] top-[0%] h-[80%] w-[65%] will-change-transform"
        style={{ background: "radial-gradient(closest-side, rgba(155,117,255,0.30), transparent 72%)" }}
      />
      <div
        className="aurora-a absolute left-[25%] top-[25%] h-[60%] w-[55%] will-change-transform"
        style={{ background: "radial-gradient(closest-side, rgba(86,139,255,0.22), transparent 72%)" }}
      />
      {intensity === "full" && (
        <>
          <div
            className="aurora-curtain aurora-curtain-a absolute left-[0%] top-[-8%] h-[62%] w-[62%] will-change-transform"
            style={{ background: curtain("66,217,232", "86,139,255") }}
          />
          <div
            className="aurora-curtain aurora-curtain-b absolute right-[-4%] top-[-4%] h-[60%] w-[55%] will-change-transform"
            style={{ background: curtain("155,117,255", "86,139,255") }}
          />
          <div
            className="aurora-curtain aurora-curtain-c absolute left-[28%] top-[-12%] h-[50%] w-[45%] will-change-transform"
            style={{ background: curtain("90,240,200", "66,217,232") }}
          />
        </>
      )}
    </div>
  );
}

/** Soft ray stripes in two hues; the curtain mask and blur turn them into diffuse light, not bars. */
function curtain(a: string, b: string): string {
  return [
    `repeating-linear-gradient(90deg, transparent 0 22px, rgba(${a},0.30) 44px, transparent 72px, rgba(${b},0.22) 100px, transparent 130px)`,
    `linear-gradient(90deg, transparent, rgba(${a},0.16) 25%, rgba(${b},0.24) 65%, transparent)`,
  ].join(", ");
}
