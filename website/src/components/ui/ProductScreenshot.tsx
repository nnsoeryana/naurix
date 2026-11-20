import Image from "next/image";
import { screenshot } from "@/config/site";

/** Hero product visual. Renders the real screenshot when configured; otherwise a clearly labelled wireframe placeholder. */
export function ProductScreenshot() {
  return (
    <div className="shot-in relative mx-auto w-full max-w-6xl">
      <div
        aria-hidden="true"
        className="absolute -inset-x-6 -bottom-10 -top-6 -z-10 rounded-[2rem] opacity-70"
        style={{
          background:
            "radial-gradient(60% 60% at 50% 40%, rgba(86,139,255,0.30), rgba(155,117,255,0.14) 55%, transparent 75%)",
        }}
      />
      <div className="overflow-hidden rounded-2xl border border-line-2 bg-bg-2 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)]">
        <div className="flex items-center gap-2 border-b border-line bg-surface px-4 py-3">
          <span className="size-3 rounded-full bg-line-2" />
          <span className="size-3 rounded-full bg-line-2" />
          <span className="size-3 rounded-full bg-line-2" />
        </div>
        {screenshot ? (
          <Image src={screenshot.src} alt={screenshot.alt} width={screenshot.width} height={screenshot.height} priority sizes="(min-width: 1152px) 1152px, 100vw" className="h-auto w-full" />
        ) : (
          <Placeholder />
        )}
      </div>
    </div>
  );
}

function Placeholder() {
  return (
    <div role="img" aria-label="Placeholder for the Naurix desktop app screenshot. Not a real screenshot." className="relative grid aspect-[16/10] grid-cols-[22%_1fr] bg-bg">
      <div className="border-r border-dashed border-line-2 p-4 sm:p-6">
        <div className="space-y-3">
          {[60, 80, 50, 70, 55].map((w, i) => (
            <div key={i} className="h-2.5 rounded-full bg-line" style={{ width: `${w}%` }} />
          ))}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3 p-4 sm:p-6">
        <div className="rounded-lg border border-dashed border-line-2" />
        <div className="grid grid-rows-2 gap-3">
          <div className="rounded-lg border border-dashed border-line-2" />
          <div className="rounded-lg border border-dashed border-line-2" />
        </div>
      </div>
      <div className="absolute inset-0 grid place-items-center">
        <div className="rounded-xl border border-line-2 bg-bg-2/95 px-5 py-4 text-center">
          <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-violet">Screenshot placeholder</p>
          <p className="mt-1.5 text-sm text-fg-2">Layout sketch, not the real UI.</p>
          <p className="mt-1 font-mono text-xs text-fg-3">Add public/screenshots/app.png, then set it in config/site.ts</p>
        </div>
      </div>
    </div>
  );
}
