import { Cpu, Gauge, GitFork, HardDrive } from "lucide-react";
import { SectionHeading } from "@/components/ui/SectionHeading";

const items = [
  {
    icon: Cpu,
    title: "macOS Intel & Apple Silicon",
    body: "Designed with compatibility across Intel and Apple Silicon Macs in mind.",
    note: "Measured on an Intel Mac. Apple Silicon validation is still in progress.",
  },
  { icon: HardDrive, title: "Local-first", body: "A workflow centered around the developer's local environment." },
  { icon: Gauge, title: "Performance-conscious", body: "Built to keep terminal and agent workflows responsive." },
  { icon: GitFork, title: "Open Source", body: "Transparent, extensible, and community-driven." },
];

export function DeveloperBenefits() {
  return (
    <section aria-labelledby="dev-title" className="bg-bg-2/50 py-24 sm:py-32">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <SectionHeading id="dev-title" eyebrow="Built for developers" title="Lightweight by design. Powerful by nature." />
        <div className="mt-16 grid gap-x-12 gap-y-12 sm:grid-cols-2">
          {items.map(({ icon: Icon, title, body, note }) => (
            <div key={title} className="reveal flex gap-5">
              <span className="grid size-11 shrink-0 place-items-center rounded-xl border border-line-2 bg-surface text-cyan">
                <Icon className="size-5" aria-hidden="true" />
              </span>
              <div>
                <h3 className="text-lg font-semibold">{title}</h3>
                <p className="mt-2 leading-relaxed text-fg-2">{body}</p>
                {note && <p className="mt-2 text-xs text-fg-3">{note}</p>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
