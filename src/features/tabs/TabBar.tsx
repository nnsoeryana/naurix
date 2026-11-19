import { IconClose, IconPlus } from "../../components/icons";
import { SHORTCUTS } from "../../shortcuts";
import { activeSpace, useStore } from "../../state/store";

const dot: Record<string, string> = {
  starting: "bg-warning",
  running: "bg-teal",
  exited: "bg-faint",
  failed: "bg-danger",
};

export function TabBar() {
  const space = useStore((s) => activeSpace(s));
  const view = useStore((s) => s.view);
  const paneState = useStore((s) => s.paneState);
  const selectTab = useStore((s) => s.selectTab);
  const closeTab = useStore((s) => s.closeTab);
  const newTab = useStore((s) => s.newTab);
  const rename = useStore((s) => s.renameTabWithPrompt);
  const hasWorkspace = useStore((s) => s.activeWorkspaceId !== null);

  return (
    // The strip is a window drag region (overlay title bar); interactive children opt out by being buttons.
    <div data-tauri-drag-region className="flex h-10 shrink-0 items-end gap-1 border-b border-line bg-surface/40 px-2" role="tablist">
      {(space?.tabs ?? []).map((t) => {
        const active = view === "terminal" && t.id === space?.activeTabId;
        const pane = paneState[t.activePaneId];
        return (
          <div
            key={t.id}
            role="tab"
            aria-selected={active}
            onClick={() => selectTab(t.id)}
            onDoubleClick={() => rename(t.id)}
            className={`group mb-[-1px] flex h-8 max-w-56 cursor-pointer items-center gap-2 rounded-t-lg border border-b-0 px-3 font-mono text-xs ${active ? "border-line bg-midnight text-cloud" : "border-transparent text-muted hover:bg-raised/50"}`}
          >
            <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${dot[pane?.kind ?? "starting"]}`} title={pane?.kind ?? "starting"} />
            <span className="truncate">{t.title}</span>
            <button
              aria-label={`Close ${t.title}`}
              className="rounded p-0.5 text-faint opacity-0 hover:bg-line hover:text-cloud group-hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                closeTab(t.id);
              }}
            >
              <IconClose className="h-3 w-3" />
            </button>
          </div>
        );
      })}
      {hasWorkspace && (
        <button className="mb-1 rounded-md p-1.5 text-muted hover:bg-raised hover:text-cloud" title={`New tab (${SHORTCUTS.newTab.label})`} aria-label="New tab" onClick={newTab}>
          <IconPlus />
        </button>
      )}
    </div>
  );
}
