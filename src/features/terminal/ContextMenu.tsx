import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SHORTCUTS } from "../../shortcuts";
import { useStore } from "../../state/store";
import { terminalHandles } from "./handles";

type Item = { label: string; hint?: string; disabled?: boolean; run: () => void } | "separator";

/** Right-click menu for a terminal pane: Copy, Paste, split in four directions, close. */
export function ContextMenu() {
  const menu = useStore((s) => s.contextMenu);
  const close = () => useStore.getState().openContextMenu(null);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });

  // Keep the menu fully inside the window.
  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    setPos({ left: Math.max(4, Math.min(menu.x, window.innerWidth - r.width - 4)), top: Math.max(4, Math.min(menu.y, window.innerHeight - r.height - 4)) });
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
    };
  }, [menu]);

  if (!menu) return null;
  const { paneId } = menu;
  const handle = terminalHandles.get(paneId);
  const st = useStore.getState();
  const split = (dir: "row" | "col", placement: "after" | "before") => () => st.splitPaneAt(paneId, dir, placement);
  const fail = (e: unknown) => useStore.getState().setNotice(String(e));

  const items: Item[] = [
    {
      label: "Copy",
      hint: "⌘C",
      disabled: !handle?.hasSelection(),
      run: () => {
        const text = handle?.getSelection() ?? "";
        if (text) invoke("clipboard_write", { text }).catch(fail);
        handle?.focus();
      },
    },
    {
      label: "Paste",
      hint: "⌘V",
      run: () =>
        void invoke<string>("clipboard_read")
          .then((text) => {
            if (text) handle?.paste(text);
            handle?.focus();
          })
          .catch(fail),
    },
    "separator",
    { label: "Split Right", hint: SHORTCUTS.splitRight.label, run: split("row", "after") },
    { label: "Split Left", run: split("row", "before") },
    { label: "Split Up", run: split("col", "before") },
    { label: "Split Down", hint: SHORTCUTS.splitDown.label, run: split("col", "after") },
    "separator",
    { label: "Close Pane", hint: SHORTCUTS.closePane.label, run: () => st.closePane(paneId) },
  ];

  return (
    <div className="fixed inset-0 z-50" onMouseDown={close} onContextMenu={(e) => (e.preventDefault(), close())}>
      <div
        ref={ref}
        role="menu"
        aria-label="Terminal menu"
        className="absolute min-w-48 rounded-lg border border-line bg-popover p-1 text-sm shadow-xl"
        style={pos}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {items.map((it, i) =>
          it === "separator" ? (
            <div key={i} className="my-1 h-px bg-line" role="separator" />
          ) : (
            <button
              key={it.label}
              role="menuitem"
              disabled={it.disabled}
              className="flex w-full items-center justify-between gap-6 rounded px-2.5 py-1 text-left hover:bg-purple/20 disabled:opacity-40 disabled:hover:bg-transparent"
              onClick={() => {
                close();
                it.run();
              }}
            >
              <span>{it.label}</span>
              {it.hint && <kbd className="text-xs text-faint">{it.hint}</kbd>}
            </button>
          ),
        )}
      </div>
    </div>
  );
}
