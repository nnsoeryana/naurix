import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { useStore } from "../../state/store";
import { contrast, isHex } from "../../themes/color";
import { exportThemeJson } from "../../themes/exportTheme";
import { themesOfKind } from "../../themes/registry";
import type { AppearanceMode, ThemeDefinition } from "../../themes/types";
import { ThemeCard } from "./ThemeCard";

const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";
const MODES: { id: AppearanceMode; label: string; hint: string }[] = [
  { id: "dark", label: "Dark", hint: "Always use the dark theme." },
  { id: "light", label: "Light", hint: "Always use the light theme." },
  { id: "system", label: "Follow system", hint: "Switch with the system light/dark setting." },
];
const ACCENT_PRESETS = ["#8b7cff", "#4f8cff", "#2ec4b6", "#f59e0b", "#ec4899", "#22c55e"];

/** Appearance (themes, light/dark/system, accent). Everything applies instantly and is persisted by Settings. */
export function AppearanceSettings() {
  const a = useStore((s) => s.settings.appearance);
  const update = useStore((s) => s.updateSettings);
  const theme = useStore((s) => s.theme);
  const setNotice = useStore((s) => s.setNotice);
  const [accentDraft, setAccentDraft] = useState(a.accent ?? "");
  const patch = (p: Partial<typeof a>) => void update((s) => ({ ...s, appearance: { ...s.appearance, ...p } }));

  /** Choosing a theme sets the slot for its kind and, unless following the system, switches to that kind. */
  const pick = (t: ThemeDefinition) =>
    patch({ ...(t.kind === "dark" ? { darkTheme: t.id } : { lightTheme: t.id }), ...(a.mode === "system" ? {} : { mode: t.kind }) });

  const setAccent = (v: string | null) => {
    setAccentDraft(v ?? "");
    patch({ accent: v });
  };

  const commitDraft = () => {
    const v = accentDraft.trim();
    if (isHex(v)) setAccent(v.toLowerCase());
    else if (v === "") setAccent(null);
    else {
      setNotice("Use a color like #8b7cff.");
      setAccentDraft(a.accent ?? "");
    }
  };

  const group = (title: string, kind: "dark" | "light") => {
    const slot = kind === "dark" ? a.darkTheme : a.lightTheme;
    return (
      <section className="mb-6" aria-label={title}>
        <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-faint">{title}</h3>
        <div role="radiogroup" aria-label={title} className="grid grid-cols-3 gap-3">
          {themesOfKind(kind).map((t) => (
            <ThemeCard key={t.id} theme={t} selected={t.id === slot || (kind !== theme.kind && false)} inUse={t.id === theme.id} note={a.mode === "system" ? `Used when the system is ${kind}` : undefined} onSelect={() => pick(t)} />
          ))}
        </div>
      </section>
    );
  };

  const ratio = isHex(a.accent) ? contrast(theme.accent, theme.tokens.background) : null;

  return (
    <>
      <div className="py-3" role="radiogroup" aria-label="Appearance mode">
        <div className="mb-2 text-sm">Appearance</div>
        <div className="inline-flex rounded-lg border border-line p-0.5">
          {MODES.map((m) => (
            <button key={m.id} role="radio" aria-checked={a.mode === m.id} title={m.hint} onClick={() => patch({ mode: m.id })} className={`rounded-md px-3 py-1 text-xs ${a.mode === m.id ? "bg-purple/25 text-cloud" : "text-muted hover:bg-raised/60"}`}>
              {a.mode === m.id ? "✓ " : ""}{m.label}
            </button>
          ))}
        </div>
        <p className="mt-1 text-[11px] text-faint">{MODES.find((m) => m.id === a.mode)?.hint} Now showing <span className="text-muted" data-active-theme>{theme.name}</span>.</p>
      </div>

      {group("Dark themes", "dark")}
      {group("Light themes", "light")}

      <div className="border-t border-line/60 py-4">
        <div className="mb-1 text-sm">Accent color</div>
        <p className="mb-3 text-[11px] text-faint">Used for selection, focus and primary buttons. It is separate from the terminal's colors, and Naurix adjusts it if needed so it stays readable on the current theme.</p>
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex gap-1.5" role="group" aria-label="Accent presets">
            {ACCENT_PRESETS.map((c) => (
              <button key={c} aria-label={`Accent ${c}`} aria-pressed={a.accent === c} onClick={() => setAccent(c)} className={`h-6 w-6 rounded-full border ${a.accent === c ? "border-cloud ring-2 ring-ring" : "border-line"}`} style={{ background: c }} />
            ))}
          </div>
          <input aria-label="Custom accent color" type="color" value={isHex(a.accent) ? a.accent : theme.accent} onChange={(e) => setAccent(e.target.value)} className="h-7 w-9 cursor-pointer rounded border border-line bg-transparent p-0.5" />
          <input aria-label="Accent hex" className="w-28 rounded border border-line bg-midnight px-2 py-1 font-mono text-xs outline-none focus:border-purple" value={accentDraft} placeholder="Theme default" onChange={(e) => setAccentDraft(e.target.value)} onBlur={commitDraft} />
          <button className={outline} disabled={a.accent === null} onClick={() => setAccent(null)}>Use theme default</button>
          <span className="btn-primary rounded-lg px-3 py-1 text-xs">Sample button</span>
        </div>
        {theme.accentAdjusted && <p className="mt-2 text-[11px] text-warning" data-accent-adjusted>Adjusted to {theme.accent} so it keeps at least 3:1 contrast against this theme's background.</p>}
        {ratio !== null && !theme.accentAdjusted && <p className="mt-2 text-[11px] text-faint">Contrast against the background: {ratio.toFixed(1)}:1.</p>}
      </div>

      <div className="border-t border-line/60 py-4">
        <div className="mb-1 text-sm">Custom themes</div>
        <p className="mb-2 text-[11px] text-faint">Importing and creating themes is planned. You can already copy the current theme as JSON, which is the format import will read.</p>
        <button className={outline} onClick={() => void invoke("clipboard_write", { text: exportThemeJson(theme) }).then(() => setNotice("Theme JSON copied to the clipboard.")).catch((e) => setNotice(String(e)))}>Copy current theme as JSON</button>
      </div>
    </>
  );
}
