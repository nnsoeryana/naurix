import { primaryCta, site, urls } from "@/config/site";
import { AuroraBackground } from "@/components/ui/AuroraBackground";
import { Button } from "@/components/ui/Button";

export function FinalCTA() {
  return (
    <section aria-labelledby="cta-title" className="relative isolate overflow-hidden py-28 sm:py-36">
      <AuroraBackground className="-z-10" />
      <div className="reveal mx-auto max-w-3xl px-4 text-center sm:px-6">
        <h2 id="cta-title" className="text-balance text-4xl font-semibold tracking-tight sm:text-5xl lg:text-6xl">
          {site.tagline}
        </h2>
        <p className="mx-auto mt-6 max-w-xl text-lg text-fg-2">
          Bring your coding agents, tools, and development sessions into one workspace.
        </p>
        <div className="mt-10 flex flex-col items-center justify-center gap-3 sm:flex-row">
          <Button href={primaryCta.href} download={primaryCta.download} external={!primaryCta.download}>{primaryCta.label}</Button>
          <Button href={urls.github} variant="secondary" external>Explore GitHub</Button>
        </div>
      </div>
    </section>
  );
}
