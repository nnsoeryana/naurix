import { Code2, GitPullRequest, Map } from "lucide-react";
import { urls } from "@/config/site";
import { Button } from "@/components/ui/Button";
import { SectionHeading } from "@/components/ui/SectionHeading";

const entries = [
  { icon: Code2, title: "Explore the Code", body: "Discover the architecture and implementation." },
  { icon: Map, title: "Shape the Roadmap", body: "Explore planned improvements and project direction." },
  { icon: GitPullRequest, title: "Contribute", body: "Help build, improve, and extend Naurix." },
];

export function OpenSource() {
  return (
    <section id="open-source" aria-labelledby="oss-title" className="py-24 sm:py-32">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <SectionHeading
          id="oss-title"
          eyebrow="Open source"
          title="Open source. Built together."
          description="Naurix is more than an application. It's an open-source project shaped by developers who believe in better AI-powered workflows."
        />
        <div className="mt-14 grid gap-5 md:grid-cols-3">
          {entries.map(({ icon: Icon, title, body }) => (
            <div key={title} className="reveal rounded-2xl border border-line bg-surface/60 p-7 transition-colors hover:border-line-2">
              <Icon className="size-6 text-violet" aria-hidden="true" />
              <h3 className="mt-5 text-lg font-semibold">{title}</h3>
              <p className="mt-2 text-fg-2">{body}</p>
            </div>
          ))}
        </div>
        <div className="reveal mt-12 flex flex-col items-center justify-center gap-3 sm:flex-row">
          <Button href={urls.github} external>Explore GitHub</Button>
          <Button href={urls.contributing} variant="secondary" external>Contribution Guide</Button>
          <Button href={urls.roadmap} variant="secondary" external>View Roadmap</Button>
        </div>
        <p className="reveal mt-6 text-center font-mono text-xs text-fg-3">Licensed under Apache-2.0</p>
      </div>
    </section>
  );
}
