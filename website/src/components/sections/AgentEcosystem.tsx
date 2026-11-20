import { agents } from "@/config/features";
import { SectionHeading } from "@/components/ui/SectionHeading";

export function AgentEcosystem() {
  return (
    <section aria-labelledby="agents-title" className="py-24 sm:py-32">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <SectionHeading id="agents-title" eyebrow="Agent ecosystem" title="Bring your favorite coding agents." description="Naurix launches the CLIs you already use, side by side, per project." />
        <ul className="mt-14 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {agents.map((a) => (
            <li key={a.name} className="reveal flex items-center justify-between rounded-xl border border-line bg-surface/60 px-6 py-5 transition-colors hover:border-line-2">
              <span className="font-mono text-base text-fg">{a.name}</span>
              <span className="text-xs text-fg-3">{a.detail}</span>
            </li>
          ))}
        </ul>
        <p className="reveal mx-auto mt-8 max-w-2xl text-center text-xs leading-relaxed text-fg-3">
          Names belong to their respective owners. Naurix is an independent project with no partnership with, or endorsement by, any of them.
        </p>
      </div>
    </section>
  );
}
