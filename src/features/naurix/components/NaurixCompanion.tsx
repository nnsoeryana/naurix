import { useStore } from "../../../state/store";
import { useNaurixMotion, useNaurixState } from "../hooks/useNaurixState";
import { NaurixVisual } from "./NaurixVisual";

/** Sidebar companion card. Decorative and informative only: not focusable, no controls. */
export function NaurixCompanion() {
  const enabled = useStore((s) => s.settings.general.naurixEnabled);
  // The inner component owns every subscription and timer, so a disabled Naurix costs nothing.
  return enabled ? <Card /> : null;
}

function Card() {
  const g = useStore((s) => s.settings.general);
  const { mode, text } = useNaurixState();
  const motion = useNaurixMotion(g.naurixAnimations, g.naurixReduced);
  return (
    <div className="nx-naurix mx-2 mb-2 flex shrink-0 items-center gap-2.5 rounded-xl border border-line bg-raised/40 p-2" data-motion={motion} title="Naurix · Your Coding Companion">
      <div className="shrink-0">
        <NaurixVisual mode={mode} size={60} />
      </div>
      <div className="min-w-0">
        <div className="text-[13px] font-semibold leading-tight text-cloud">Naurix</div>
        <p role="status" className="text-[11px] leading-snug text-muted">{text}</p>
      </div>
    </div>
  );
}
