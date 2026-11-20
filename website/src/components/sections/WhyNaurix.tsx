import { Blocks, HandMetal, LayoutGrid } from "lucide-react";
import { SectionHeading } from "@/components/ui/SectionHeading";

const principles = [
  { icon: LayoutGrid, title: "One Workspace", body: "Bring projects, terminal sessions, and coding agents together." },
  { icon: Blocks, title: "Your Agents, Your Choice", body: "Use the coding agents that fit your workflow." },
  { icon: HandMetal, title: "Human in Control", body: "Keep control over execution, integrations, and your development environment." },
];

export function WhyNaurix() {
  return (
    <section aria-labelledby="why-title" className="relative py-24 sm:py-32">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <SectionHeading
          id="why-title"
          eyebrow="Why Naurix"
          title="AI should expand your capabilities, not take away your control."
          description="Developers deserve tools that make AI workflows more powerful without making them more complicated."
        />
        <div className="mt-16 grid gap-px overflow-hidden rounded-2xl border border-line bg-line md:grid-cols-3">
          {principles.map(({ icon: Icon, title, body }, i) => (
            <div key={title} className="reveal relative bg-bg-2 p-8" style={{ transitionDelay: `${i * 80}ms` }}>
              <span aria-hidden="true" className="absolute inset-x-8 top-0 h-px bg-gradient-to-r from-cyan/0 via-cyan/60 to-violet/0" />
              <Icon className="size-6 text-blue" aria-hidden="true" />
              <h3 className="mt-6 text-xl font-semibold">{title}</h3>
              <p className="mt-3 leading-relaxed text-fg-2">{body}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
