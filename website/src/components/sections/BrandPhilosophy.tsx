import { AuroraBackground } from "@/components/ui/AuroraBackground";

const meanings = ["Light", "Inspiration", "Energy", "Possibilities", "Creativity"];

export function BrandPhilosophy() {
  return (
    <section aria-labelledby="brand-title" className="relative isolate overflow-hidden bg-bg-2/50 py-24 sm:py-32">
      <AuroraBackground className="-z-10" intensity="soft" />
      <div className="reveal mx-auto max-w-3xl px-4 text-center sm:px-6">
        <p className="font-mono text-xs uppercase tracking-[0.2em] text-cyan">Brand</p>
        <h2 id="brand-title" className="mt-4 text-balance text-3xl font-semibold tracking-tight sm:text-4xl lg:text-5xl">
          Inspired by light. Built for possibilities.
        </h2>
        <p className="mt-8 leading-relaxed text-fg-2">
          Naurix began as a personal idea and grew into an open-source project for developers everywhere. Aurora stands for what we want the tool to feel like:
        </p>
        <ul className="mt-8 flex flex-wrap justify-center gap-2">
          {meanings.map((m) => (
            <li key={m} className="rounded-full border border-line-2 bg-surface/70 px-4 py-1.5 text-sm text-fg-2">{m}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}
