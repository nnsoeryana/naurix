import { useEffect } from "react";
import { DialogHost } from "./components/Dialogs";
import { AgentLauncher } from "./features/agents/AgentLauncher";
import { PaneTree } from "./features/layout/PaneTree";
import { runShortcut } from "./features/palette/actions";
import { CommandPalette } from "./features/palette/CommandPalette";
import { TabBar } from "./features/tabs/TabBar";
import { Home } from "./features/home/Home";
import { GitPanel } from "./features/git/GitPanel";
import { branchLabel, dirtyCount } from "./features/git/api";
import { McpManager } from "./features/mcp/McpManager";
import { SettingsView } from "./features/settings/SettingsView";
import { SkillsManager } from "./features/skills/SkillsManager";
import { ContextMenu } from "./features/terminal/ContextMenu";
import { Sidebar } from "./features/workspaces/Sidebar";
import { SHORTCUTS, clipboardShortcut, matchShortcut, tabIndexShortcut } from "./shortcuts";
import { copyPane, pastePane } from "./features/terminal/clipboard";
import { applyTheme, cacheAppearance } from "./themes/apply";
import { activeSpace, useStore } from "./state/store";

export default function App() {
  const init = useStore((s) => s.init);
  const loaded = useStore((s) => s.loaded);

  useEffect(() => {
    void init().then(() => {
      void import("./bench").then((m) => m.runBenchMode());
      if (import.meta.env.DEV) void import("./dev/smoke").then((m) => m.runSmoke());
    });
  }, [init]);

  // Theme: publish CSS variables once per theme change (not per render) and follow the OS appearance.
  const theme = useStore((s) => s.theme);
  const customAccent = useStore((s) => s.settings.appearance.accent);
  useEffect(() => {
    applyTheme(document.documentElement, theme);
    cacheAppearance({ id: theme.id, accent: customAccent });
  }, [theme, customAccent]);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => useStore.getState().setSystemDark(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  // Git status is refreshed on events only (never on a timer): when the window regains focus.
  useEffect(() => {
    const onFocus = () => void useStore.getState().refreshGit();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // Capture phase: win against xterm.js, which otherwise sees the key first.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const st = useStore.getState();
      const idx = tabIndexShortcut(e);
      const id = matchShortcut(e);
      const clip = clipboardShortcut(e);
      if (idx === null && id === null && clip === null) return;
      e.preventDefault();
      e.stopPropagation();
      if (st.dialog || st.launcherOpen || st.contextMenu) return; // modal dialogs own the keyboard
      if (clip) {
        const sp = activeSpace(st);
        const pane = sp?.tabs.find((t) => t.id === sp.activeTabId)?.activePaneId;
        if (pane) (clip === "copy" ? copyPane : pastePane)(pane);
        return;
      }
      if (idx !== null) st.selectTabIndex(idx);
      else if (id) runShortcut(id);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  if (!loaded) return null;
  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <Main />
      </div>
      <StatusBar />
      <CommandPalette />
      <ContextMenu />
      <AgentLauncher />
      <DialogHost />
    </div>
  );
}

function Main() {
  const activeId = useStore((s) => s.activeWorkspaceId);
  const spaces = useStore((s) => s.spaces);
  const view = useStore((s) => s.view);
  const space = activeId ? spaces[activeId] : undefined;
  // Home shows when chosen, and whenever the workspace has nothing else to show.
  const showMcp = view === "mcp";
  const showSkills = view === "skills";
  const showGit = view === "git";
  const showSettings = view === "settings";
  const showHome = !showMcp && !showSkills && !showGit && !showSettings && (view === "home" || !space || space.tabs.length === 0);

  return (
    <main className="flex min-w-0 flex-1 flex-col">
      <TabBar />
      <div className="relative min-h-0 flex-1 bg-terminal">
        {/* Every tab of every workspace stays mounted so terminals keep their buffers; hidden ones use display:none. */}
        {Object.entries(spaces).flatMap(([wsId, sp]) =>
          sp.tabs.map((t) => <PaneTree key={t.id} wsId={wsId} tab={t} visible={!showHome && !showMcp && !showSkills && !showGit && !showSettings && wsId === activeId && t.id === sp.activeTabId} />),
        )}
        {showHome && <Home />}
        {showMcp && <McpManager />}
        {showSkills && <SkillsManager />}
        {showGit && <GitPanel />}
        {showSettings && <SettingsView />}
      </div>
    </main>
  );
}

function StatusBar() {
  const notice = useStore((s) => s.notice);
  const setNotice = useStore((s) => s.setNotice);
  const workspace = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId));
  const tab = useStore((s) => {
    const sp = activeSpace(s);
    return sp?.tabs.find((t) => t.id === sp.activeTabId);
  });
  const state = useStore((s) => (tab ? s.paneState[tab.activePaneId] : undefined));
  const gitResult = useStore((s) => s.git.result);
  const setView = useStore((s) => s.setView);
  const label = !tab ? "" : !state ? "Starting…" : state.kind === "exited" ? `Exited (${state.signal ?? `code ${state.code ?? "?"}`})` : state.kind === "failed" ? "Failed" : state.kind === "running" ? "Running" : "Starting…";

  return (
    <footer className="flex items-center gap-3 border-t border-line bg-surface/40 px-3 py-1 text-xs text-muted">
      {notice ? (
        <>
          <span className="truncate text-warning">{notice}</span>
          <button className="rounded px-1 hover:bg-raised" onClick={() => setNotice(null)}>Dismiss</button>
        </>
      ) : (
        <>
          <span>{workspace?.name ?? "No workspace"}</span>
          {tab && <span>· {tab.title}</span>}
          {label && <span>· {label}</span>}
          {gitResult?.state === "repo" && (
            <button className="rounded px-1.5 font-mono hover:bg-raised" onClick={() => setView("git")} title="Open Git status" data-status-branch>
              ⎇ {branchLabel(gitResult)}{dirtyCount(gitResult) > 0 ? ` ●${dirtyCount(gitResult)}` : ""}
            </button>
          )}
          <span className="ml-auto">{SHORTCUTS.palette.label} Commands</span>
        </>
      )}
    </footer>
  );
}
