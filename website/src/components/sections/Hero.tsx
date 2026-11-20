import { primaryCta, urls } from "@/config/site";
import { AuroraBackground } from "@/components/ui/AuroraBackground";
import { Button } from "@/components/ui/Button";
import { ProductScreenshot } from "@/components/ui/ProductScreenshot";

export function Hero() {
  return (
    <section id="product" aria-labelledby="hero-title" className="relative isolate overflow-hidden pb-24 pt-16 sm:pt-24">
      <AuroraBackground className="-z-10" />
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <div className="mx-auto max-w-4xl text-center">
          <h1 id="hero-title" className="text-balance text-4xl font-semibold leading-[1.05] tracking-tight sm:text-6xl lg:text-7xl">
            Your AI coding agents.
            <br />
            <span className="text-aurora">One command center.</span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-pretty text-base leading-relaxed text-fg-2 sm:text-lg">
            Manage your terminals, AI coding agents, integrations, and development workflows in one lightweight workspace built for developers.
          </p>
          <div className="mt-9 flex flex-col items-center justify-center gap-3 sm:flex-row">
            <Button href={primaryCta.href} download={primaryCta.download} external={!primaryCta.download}>
              {primaryCta.label}
            </Button>
            <Button href={urls.github} variant="secondary" external>
              Explore GitHub
            </Button>
          </div>
          <p className="mt-6 font-mono text-xs text-fg-3">Open Source · macOS · Intel &amp; Apple Silicon targets</p>
          {!primaryCta.download && (
            <p className="mt-2 text-xs text-fg-3">No packaged release yet. Naurix is in early development; build it from source.</p>
          )}
        </div>
        <div className="mt-16 sm:mt-20">
          <ProductScreenshot />
        </div>
      </div>
    </section>
  );
}
