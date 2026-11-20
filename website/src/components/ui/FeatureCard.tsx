import { statusLabel, type Feature } from "@/config/features";

export function FeatureCard({ feature, showStatus }: { feature: Feature; showStatus: boolean }) {
  const { icon: Icon, title, description, note, status } = feature;
  return (
    <article className="reveal group relative flex flex-col rounded-2xl border border-line bg-surface/70 p-6 transition-[transform,border-color,background-color] duration-300 hover:-translate-y-1 hover:border-line-2 hover:bg-elevated/80">
      <span className="mb-5 grid size-11 place-items-center rounded-xl border border-line-2 bg-bg-2 text-cyan transition-[box-shadow,color] duration-300 group-hover:text-fg group-hover:shadow-[0_0_24px_-2px_rgba(66,217,232,0.55)]">
        <Icon className="size-5" aria-hidden="true" />
      </span>
      <div className="flex items-center gap-3">
        <h3 className="text-lg font-semibold">{title}</h3>
        {showStatus && status !== "available" && (
          <span className="rounded-full border border-violet/40 px-2 py-0.5 text-[11px] font-medium text-violet">
            {statusLabel[status]}
          </span>
        )}
      </div>
      <p className="mt-2 text-sm leading-relaxed text-fg-2">{description}</p>
      {note && <p className="mt-4 border-t border-line pt-4 text-xs leading-relaxed text-fg-3">{note}</p>}
    </article>
  );
}
