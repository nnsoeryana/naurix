import { AuroraBackground } from "@/components/ui/AuroraBackground";
import { NaurixVisual } from "@/components/ui/NaurixVisual";
import { SectionHeading } from "@/components/ui/SectionHeading";

export function Naurix() {
  return (
    <section id="naurix" aria-labelledby="naurix-title" className="relative isolate overflow-hidden py-24 sm:py-32">
      <AuroraBackground className="-z-10" intensity="soft" />
      <div className="mx-auto grid max-w-7xl items-center gap-12 px-4 sm:px-6 lg:grid-cols-2 lg:gap-8 lg:px-8">
        <div className="lg:order-2">
          <SectionHeading
            id="naurix-title"
            align="left"
            eyebrow="Meet Naurix"
            title={
              <>
                Meet Naurix. <span className="text-aurora">The light behind the workflow.</span>
              </>
            }
            description="An Aurora-inspired companion representing the energy, creativity, and possibilities behind Naurix."
          />
          <p className="reveal mt-6 text-sm leading-relaxed text-fg-3">
            This animation is a brand illustration. It is not connected to your agents or to the desktop app.
          </p>
        </div>
        <div className="reveal mx-auto w-full max-w-xl lg:order-1">
          <NaurixVisual className="h-auto w-full" />
        </div>
      </div>
    </section>
  );
}
