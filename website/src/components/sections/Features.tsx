import { features } from "@/config/features";
import { FeatureCard } from "@/components/ui/FeatureCard";
import { SectionHeading } from "@/components/ui/SectionHeading";

export function Features() {
  // Status labels appear only when something is not yet available.
  const showStatus = features.some((f) => f.status !== "available");
  return (
    <section id="features" aria-labelledby="features-title" className="bg-bg-2/50 py-24 sm:py-32">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <SectionHeading id="features-title" eyebrow="Features" title="Everything your agents need." description="What works in the app today, as of milestone M10 of 11." />
        <div className="mt-16 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <FeatureCard key={f.title} feature={f} showStatus={showStatus} />
          ))}
        </div>
      </div>
    </section>
  );
}
