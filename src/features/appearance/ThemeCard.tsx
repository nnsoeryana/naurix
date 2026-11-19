import type { ThemeDefinition } from "../../themes/types";

/**
 * A selectable preview of one theme. It paints itself from the theme's own tokens (not the active
 * CSS variables), so every card looks like its theme whatever is currently applied.
 */
export function ThemeCard({ theme, selected, inUse, note, onSelect }: { theme: ThemeDefinition; selected: boolean; inUse: boolean; note?: string; onSelect: () => void }) {
  const k = theme.tokens;
  const ansi = k.terminal.ansi;
  const swatches = [ansi.red, ansi.green, ansi.yellow, ansi.blue, ansi.magenta, ansi.cyan];
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      data-theme-card={theme.id}
      className={`group rounded-xl border p-2 text-left transition-colors ${selected ? "border-purple bg-purple/10" : "border-line hover:bg-raised/60"}`}
    >
      <div className="overflow-hidden rounded-lg border" style={{ background: k.background, borderColor: k.border }} aria-hidden>
        <div className="flex h-16">
          <div className="w-1/4 space-y-1 p-1.5" style={{ background: k.sidebarBackground, borderRight: `1px solid ${k.border}` }}>
            <div className="h-1.5 w-3/4 rounded" style={{ background: k.accent }} />
            <div className="h-1 w-full rounded" style={{ background: k.surfaceSecondary }} />
            <div className="h-1 w-2/3 rounded" style={{ background: k.surfaceSecondary }} />
          </div>
          <div className="flex-1 p-1.5" style={{ background: k.terminal.background }}>
            <div className="font-mono text-[9px] leading-tight" style={{ color: k.terminal.foreground }}>
              <span style={{ color: k.accentSecondary }}>❯</span> naurix
            </div>
            <div className="mt-1 flex gap-1">{swatches.map((c, i) => <span key={i} className="h-2 w-2 rounded-sm" style={{ background: c }} />)}</div>
            <div className="mt-1.5 inline-block rounded px-1.5 text-[8px] font-semibold" style={{ background: k.accent, color: k.accentForeground }}>Aa</div>
          </div>
        </div>
      </div>
      <div className="mt-2 flex items-center justify-between gap-2 px-0.5">
        <span className="truncate text-sm">{theme.name}</span>
        <span className="shrink-0 text-[10px] uppercase tracking-wide text-faint">{theme.kind}</span>
      </div>
      <div className="h-4 px-0.5 text-[11px] text-muted">{selected ? `✓ Selected${inUse ? " · in use" : ""}` : note ?? ""}</div>
    </button>
  );
}
