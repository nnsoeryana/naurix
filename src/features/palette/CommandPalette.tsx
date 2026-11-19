import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../../state/store";
import { buildActions } from "./actions";

export function CommandPalette() {
  const open = useStore((s) => s.paletteOpen);
  const setPalette = useStore((s) => s.setPalette);
  if (!open) return null;
  return <Palette close={() => setPalette(false)} />;
}

function Palette({ close }: { close: () => void }) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const actions = useMemo(() => buildActions(), []);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? actions.filter((a) => a.title.toLowerCase().includes(q)) : actions;
  }, [actions, query]);
  const cur = Math.min(index, Math.max(0, shown.length - 1));

  useEffect(() => input.current?.focus(), []);

  const run = (i: number) => {
    const a = shown[i];
    if (!a) return;
    close();
    // Let focus return to the terminal before the action runs.
    setTimeout(a.run, 0);
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-overlay pt-24" onMouseDown={close}>
      <div className="w-[34rem] overflow-hidden rounded-lg border border-line bg-dialog shadow-xl" role="dialog" aria-label="Command palette" onMouseDown={(e) => e.stopPropagation()}>
        <input
          ref={input}
          value={query}
          placeholder="Type a command…"
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") close();
            else if (e.key === "ArrowDown") (e.preventDefault(), setIndex((cur + 1) % Math.max(1, shown.length)));
            else if (e.key === "ArrowUp") (e.preventDefault(), setIndex((cur - 1 + shown.length) % Math.max(1, shown.length)));
            else if (e.key === "Enter") (e.preventDefault(), run(cur));
          }}
          className="w-full border-b border-line bg-transparent px-3 py-2 text-sm outline-none"
        />
        <ul className="max-h-80 overflow-auto py-1">
          {shown.length === 0 && <li className="px-3 py-2 text-sm text-faint">No matching commands</li>}
          {shown.map((a, i) => (
            <li
              key={a.id}
              onMouseMove={() => setIndex(i)}
              onClick={() => run(i)}
              className={`flex cursor-pointer items-center justify-between px-3 py-1.5 text-sm ${i === cur ? "bg-purple/20" : ""}`}
            >
              <span>{a.title}</span>
              {a.shortcut && <kbd className="text-xs text-faint">{a.shortcut}</kbd>}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
