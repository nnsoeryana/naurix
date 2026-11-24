import { create } from "zustand";
import { cyclePane, hasPane, leaf, paneIds, removePane, setRatio, splitPane, type Layout, type Placement, type SplitDir } from "../features/layout/layout";
import type { TerminalState } from "../features/terminal/TerminalView";
import { sessionRegistry } from "../features/terminal/registry";
import { sessionApi, terminalApi, type LiveSession } from "../features/terminal/api";
import { planReattach, type ReattachItem } from "../features/terminal/reattach";
import { tildePath, uniqueTitle } from "../features/workspaces/path";
import { workspaceApi, type SessionRecord, type Workspace } from "../features/workspaces/api";
import { agentApi, type AgentInfo } from "../features/agents/api";
import { isAgentSignalKind, type AgentSignal } from "../features/agents/status";
import { gitApi, type StatusResult } from "../features/git/api";
import { DEFAULT_SETTINGS, settingsApi, type Settings } from "../features/settings/api";
import { resolveAppearance, type ResolvedTheme } from "../themes/resolve";

/**
 * UI state only (ADR: Rust owns process truth, SQLite owns durable metadata). Terminal bytes
 * never pass through here.
 */
export type Tab = { id: string; title: string; layout: Layout; activePaneId: string };
/** `sessionId` is set when the pane reattaches to a session Core kept alive across a reload (ADR 0026). */
export type PaneKind = { type: "shell"; atRepoRoot?: boolean; /** Open in a worktree Naurix created (resolved by the backend). */ worktreeId?: string; sessionId?: number } | { type: "agent"; agentId: string; name: string; /** Start in a fresh worktree (ADR 0028). */ worktree?: boolean; /** Branch of the worktree it runs in, once known. */ branch?: string; sessionId?: number };
export type Space = { tabs: Tab[]; activeTabId: string | null };
export type Dialog =
  | { kind: "confirm"; title: string; message: string; confirmLabel: string; danger?: boolean; onConfirm: () => void }
  | { kind: "prompt"; title: string; initial: string; onSubmit: (value: string) => void };

/** The OS appearance (macOS Dark/Light), as seen by the web view. */
function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)").matches : true;
}

let counter = 0;
let settingsQueue: Promise<void> = Promise.resolve();
let reattachInflight: Promise<void> | null = null;
let gitInflight = false;
let gitPending = false;
const uid = (prefix: string) => `${prefix}${++counter}`;

export type View = "home" | "terminal" | "mcp" | "skills" | "git" | "workflows" | "memory" | "settings";

type State = {
  loaded: boolean;
  view: View;
  workspaces: Workspace[];
  activeWorkspaceId: string | null;
  spaces: Record<string, Space>;
  paneState: Record<string, TerminalState>;
  paneGen: Record<string, number>;
  paneKinds: Record<string, PaneKind>;
  /** Last status an agent reported, by backend session id (ADR 0029). Absent means no signal. */
  sessionStatus: Record<number, AgentSignal>;
  setSessionStatus: (session: number, status: AgentSignal | null) => void;
  agents: AgentInfo[];
  launcherOpen: boolean;
  settings: Settings;
  /** The theme in effect: derived from settings.appearance and the OS appearance (the only copy). */
  theme: ResolvedTheme;
  systemDark: boolean;
  setSystemDark: (dark: boolean) => void;
  syncTheme: () => void;
  rendererStatus: "dom" | "webgl-active" | "webgl-unavailable";
  setRendererStatus: (s: "dom" | "webgl-active" | "webgl-unavailable") => void;
  updateSettings: (mutate: (s: Settings) => Settings) => Promise<void>;
  resetSettings: () => Promise<void>;
  previous: Record<string, SessionRecord[]>;
  paletteOpen: boolean;
  dialog: Dialog | null;
  notice: string | null;

  setView: (v: View) => void;
  git: { workspaceId: string | null; result: StatusResult | null; error: string | null; loading: boolean };
  refreshGit: () => Promise<void>;
  trustRepo: () => Promise<void>;
  newRepoTab: (title: string) => void;
  init: () => Promise<void>;
  /** Bring back, as one tab each, the sessions Core kept running while the page was reloaded. */
  reattachSessions: () => Promise<void>;
  /** Show a session Core already runs (for example a workflow step) in a pane, or reveal the pane that has it. */
  openSession: (sessionId: number) => Promise<void>;
  /** Incremented when a workflow run changes (ADR 0030); views re-read their runs. */
  runTick: number;
  bumpRuns: () => void;
  addWorkspace: () => Promise<void>;
  selectWorkspace: (id: string) => void;
  /** User picked a project: select it and, if it has no terminal yet, open one (unless a tool screen is showing). */
  openWorkspace: (id: string) => void;
  removeWorkspaceWithConfirm: (id: string) => void;
  renameWorkspaceWithPrompt: (id: string) => void;
  newTab: () => void;
  splitActive: (dir: SplitDir) => void;
  splitPaneAt: (paneId: string, dir: SplitDir, placement: Placement) => void;
  /** Which pane shows its find bar (TERM-010). Holds no query: that stays inside the FindBar. `nonce` re-focuses an open bar. */
  find: { paneId: string; nonce: number } | null;
  openFind: (paneId: string) => void;
  closeFind: () => void;
  contextMenu: { x: number; y: number; paneId: string } | null;
  openContextMenu: (m: { x: number; y: number; paneId: string } | null) => void;
  closeActivePane: () => void;
  closePane: (paneId: string) => void;
  closeTab: (tabId: string) => void;
  selectTab: (tabId: string) => void;
  selectTabIndex: (i: number) => void;
  stepTab: (delta: 1 | -1) => void;
  stepPane: (delta: 1 | -1) => void;
  focusPane: (paneId: string) => void;
  renameTabWithPrompt: (tabId?: string) => void;
  setRatio: (tabId: string, splitId: string, ratio: number) => void;
  setPaneState: (paneId: string, s: TerminalState) => void;
  restartPane: (paneId: string) => void;
  refreshAgents: () => Promise<void>;
  setAgents: (a: AgentInfo[]) => void;
  openLauncher: (open: boolean) => void;
  /** Open a tab running this agent. The backend refuses unless an approval exists (see the launcher). */
  launchAgent: (agentId: string, opts?: { worktree?: boolean }) => void;
  /** Open a terminal in a worktree Naurix created. The backend resolves the folder from its id. */
  newWorktreeTab: (title: string, worktreeId: string) => void;
  setPaneBranch: (paneId: string, branch: string) => void;
  revealPane: (paneId: string) => void;
  setFontFamilyWithPrompt: () => void;
  setFontSizeWithPrompt: () => void;
  setPalette: (open: boolean) => void;
  setDialog: (d: Dialog | null) => void;
  setNotice: (n: string | null) => void;
};

const activeSpace = (s: State): Space | undefined => (s.activeWorkspaceId ? s.spaces[s.activeWorkspaceId] : undefined);
const activeTab = (s: State): Tab | undefined => activeSpace(s)?.tabs.find((t) => t.id === activeSpace(s)!.activeTabId);

function locatePane(s: State, paneId: string): { wsId: string; tab: Tab } | null {
  for (const [wsId, space] of Object.entries(s.spaces)) {
    const tab = space.tabs.find((t) => hasPane(t.layout, paneId));
    if (tab) return { wsId, tab };
  }
  return null;
}

async function anyForegroundJob(paneIdList: string[], isRunningAgent: (paneId: string) => boolean): Promise<boolean> {
  const checks = paneIdList.map(async (p) => {
    // An agent is its own foreground process, so "running" is the signal that closing would stop work.
    if (isRunningAgent(p)) return true;
    const sid = sessionRegistry.get(p);
    return sid === undefined ? false : terminalApi.hasForegroundJob(sid).catch(() => false);
  });
  return (await Promise.all(checks)).some(Boolean);
}

export const useStore = create<State>()((set, get) => {
  const patchSpace = (wsId: string, fn: (sp: Space) => Space) =>
    set((s) => ({ spaces: { ...s.spaces, [wsId]: fn(s.spaces[wsId] ?? { tabs: [], activeTabId: null }) } }));

  /** One tab for a session Core already runs, with the status it last reported (ADR 0026, 0029). */
  const adopt = (it: ReattachItem, live: LiveSession[]) => {
    const paneId = uid("p");
    const tabId = uid("t");
    const title = uniqueTitle(it.title, (get().spaces[it.workspaceId]?.tabs ?? []).map((t) => t.title));
    set((st) => ({ paneKinds: { ...st.paneKinds, [paneId]: it.kind } }));
    const reported = live.find((l) => l.id === it.sessionId)?.status;
    if (reported && isAgentSignalKind(reported.kind)) get().setSessionStatus(it.sessionId, { kind: reported.kind, at: reported.at });
    patchSpace(it.workspaceId, (sp) => ({ tabs: [...sp.tabs, { id: tabId, title, layout: leaf(paneId), activePaneId: paneId }], activeTabId: sp.activeTabId ?? tabId }));
  };
  const loadPrevious = (wsId: string) =>
    workspaceApi.recentSessions(wsId).then((rows) => set((s) => ({ previous: { ...s.previous, [wsId]: rows } }))).catch(() => {});

  const fail = (e: unknown) => set({ notice: String(e) });

  /**
   * The user closed this pane on purpose: stop its process and let Core forget the session. This is the
   * only place a session is ended from the UI; a view that merely unmounts only detaches (ADR 0026).
   */
  const endSession = (paneId: string) => {
    const sid = sessionRegistry.get(paneId);
    if (sid !== undefined) sessionApi.close(sid).catch(() => {});
    sessionRegistry.delete(paneId);
  };

  /** Close after asking for confirmation if a command is still running in these panes. */
  const confirmIfBusy = async (panes: string[], title: string, message: string, then: () => void) => {
    const runningAgent = (p: string) => {
      const st = get();
      const k = st.paneKinds[p];
      const ps = st.paneState[p]?.kind;
      return k?.type === "agent" && (ps === "running" || ps === "starting");
    };
    if (get().settings.general.confirmCloseRunning && (await anyForegroundJob(panes, runningAgent))) {
      set({ dialog: { kind: "confirm", title, message, confirmLabel: "Close", danger: true, onConfirm: then } });
    } else {
      then();
    }
  };

  return {
    loaded: false,
    view: "home" as View,
    git: { workspaceId: null, result: null, error: null, loading: false },
    workspaces: [],
    activeWorkspaceId: null,
    spaces: {},
    paneState: {},
    paneGen: {},
    paneKinds: {},
    sessionStatus: {},
    setSessionStatus: (session, status) =>
      set((st) => {
        if (!status) {
          if (!(session in st.sessionStatus)) return {};
          const { [session]: _gone, ...rest } = st.sessionStatus;
          return { sessionStatus: rest };
        }
        return { sessionStatus: { ...st.sessionStatus, [session]: status } };
      }),
    agents: [],
    launcherOpen: false,
    settings: DEFAULT_SETTINGS,
    theme: resolveAppearance(DEFAULT_SETTINGS.appearance, systemPrefersDark()),
    systemDark: systemPrefersDark(),
    setSystemDark: (systemDark) => {
      set({ systemDark });
      get().syncTheme();
    },
    syncTheme: () => {
      const next = resolveAppearance(get().settings.appearance, get().systemDark);
      const cur = get().theme;
      // Only publish a new theme object when something visible changed, so nothing re-renders needlessly.
      if (next.id !== cur.id || next.accent !== cur.accent || next.gradientButtons !== cur.gradientButtons) set({ theme: next });
    },
    rendererStatus: "dom" as const,
    setRendererStatus: (rendererStatus) => set({ rendererStatus }),
    previous: {},
    paletteOpen: false,
    dialog: null,
    notice: null,

    refreshGit: async () => {
      // Coalesced: at most one request in flight plus one queued. Called on events, never on a timer.
      if (gitInflight) {
        gitPending = true;
        return;
      }
      const wsId = get().activeWorkspaceId;
      if (!wsId) return set({ git: { workspaceId: null, result: null, error: null, loading: false } });
      gitInflight = true;
      set((s) => ({ git: { ...s.git, workspaceId: wsId, loading: true } }));
      try {
        const result = await gitApi.status(wsId);
        if (get().activeWorkspaceId === wsId) set({ git: { workspaceId: wsId, result, error: null, loading: false } });
      } catch (e) {
        if (get().activeWorkspaceId === wsId) set({ git: { workspaceId: wsId, result: null, error: String(e), loading: false } });
      } finally {
        gitInflight = false;
        if (gitPending) {
          gitPending = false;
          void get().refreshGit();
        }
      }
    },
    trustRepo: async () => {
      const wsId = get().activeWorkspaceId;
      if (!wsId) return;
      try {
        set({ git: { workspaceId: wsId, result: await gitApi.trust(wsId), error: null, loading: false } });
      } catch (e) {
        fail(e);
      }
    },
    newRepoTab: (title) => {
      const s = get();
      const ws = s.workspaces.find((w) => w.id === s.activeWorkspaceId);
      if (!ws?.available) return;
      const paneId = uid("p");
      const tabId = uid("t");
      set((st) => ({ view: "terminal", paneKinds: { ...st.paneKinds, [paneId]: { type: "shell", atRepoRoot: true } } }));
      const unique = uniqueTitle(title, (s.spaces[ws.id]?.tabs ?? []).map((t) => t.title));
      patchSpace(ws.id, (sp) => ({ tabs: [...sp.tabs, { id: tabId, title: unique, layout: leaf(paneId), activePaneId: paneId }], activeTabId: tabId }));
    },

    newWorktreeTab: (title, worktreeId) => {
      const s = get();
      const ws = s.workspaces.find((w) => w.id === s.activeWorkspaceId);
      if (!ws?.available) return;
      const paneId = uid("p");
      const tabId = uid("t");
      set((st) => ({ view: "terminal", paneKinds: { ...st.paneKinds, [paneId]: { type: "shell", worktreeId } } }));
      const unique = uniqueTitle(title, (s.spaces[ws.id]?.tabs ?? []).map((t) => t.title));
      patchSpace(ws.id, (sp) => ({ tabs: [...sp.tabs, { id: tabId, title: unique, layout: leaf(paneId), activePaneId: paneId }], activeTabId: tabId }));
    },
    setPaneBranch: (paneId, branch) =>
      set((st) => {
        const k = st.paneKinds[paneId];
        return k?.type === "agent" ? { paneKinds: { ...st.paneKinds, [paneId]: { ...k, branch } } } : {};
      }),

    setView: (view) => {
      set({ view });
      if (view === "git") void get().refreshGit();
    },

    init: async () => {
      try {
        const workspaces = await workspaceApi.list();
        const first = workspaces[0]?.id ?? null;
        set({ workspaces, activeWorkspaceId: first, loaded: true });
        void get().refreshAgents();
        void get().refreshGit();
        settingsApi.get().then((settings) => (set({ settings }), get().syncTheme())).catch(() => {});
        await get().reattachSessions();
        await Promise.all(workspaces.map((w) => loadPrevious(w.id)));
      } catch (e) {
        set({ loaded: true, notice: `Could not load workspaces: ${e}` });
      }
    },

    reattachSessions: () => {
      // init() can run more than once (React StrictMode in development), and two concurrent runs would
      // each add a tab for the same session. One run at a time, and never a second pane for a session.
      reattachInflight ??= (async () => {
        try {
          const live = await sessionApi.list();
          const known = new Set(get().workspaces.map((w) => w.id));
          const have = sessionRegistry.ids();
          for (const k of Object.values(get().paneKinds)) if (k.sessionId !== undefined) have.add(k.sessionId);
          for (const it of planReattach(live, known)) {
            if (have.has(it.sessionId)) continue;
            adopt(it, live);
          }
          const active = get().activeWorkspaceId;
          if (active && (get().spaces[active]?.tabs.length ?? 0) > 0) set({ view: "terminal" });
        } catch (e) {
          fail(e);
        }
      })().finally(() => {
        reattachInflight = null;
      });
      return reattachInflight;
    },

    runTick: 0,
    bumpRuns: () => set((s) => ({ runTick: s.runTick + 1 })),
    openSession: async (sessionId) => {
      try {
        const own = Object.entries(get().paneKinds).find(([p, k]) => (sessionRegistry.get(p) ?? k.sessionId) === sessionId);
        if (own) {
          get().revealPane(own[0]);
          return;
        }
        const live = await sessionApi.list();
        const known = new Set(get().workspaces.map((w) => w.id));
        for (const it of planReattach(live.filter((l) => l.id === sessionId), known)) adopt(it, live);
        set({ view: "terminal" });
      } catch (e) {
        fail(e);
      }
    },

    addWorkspace: async () => {
      try {
        const path = await workspaceApi.pickFolder();
        if (!path) return;
        const w = await workspaceApi.create(path);
        set((s) => ({ workspaces: s.workspaces.some((x) => x.id === w.id) ? s.workspaces : [w, ...s.workspaces], activeWorkspaceId: w.id }));
        workspaceApi.open(w.id).catch(() => {});
        loadPrevious(w.id);
      } catch (e) {
        fail(e);
      }
    },

    selectWorkspace: (id) => {
      set((st) => ({ activeWorkspaceId: id, view: ["mcp", "skills", "git", "workflows", "memory", "settings"].includes(st.view) ? st.view : (st.spaces[id]?.tabs.length ?? 0) > 0 ? "terminal" : "home" }));
      workspaceApi.open(id).catch(() => {});
      void get().refreshGit();
      // Refresh availability (the folder may have been moved or deleted).
      workspaceApi.list().then((workspaces) => set({ workspaces })).catch(() => {});
    },

    openWorkspace: (id) => {
      get().selectWorkspace(id);
      const s = get();
      const empty = (s.spaces[id]?.tabs.length ?? 0) === 0;
      if (empty && (s.view === "home" || s.view === "terminal")) get().newTab();
    },

    removeWorkspaceWithConfirm: (id) => {
      const w = get().workspaces.find((x) => x.id === id);
      if (!w) return;
      set({
        dialog: {
          kind: "confirm",
          title: `Remove “${w.name}”?`,
          message: "Its terminals will be stopped. Your project files are not deleted.",
          confirmLabel: "Remove",
          danger: true,
          onConfirm: () => {
            workspaceApi
              .remove(id)
              .then(() =>
                set((s) => {
                  const { [id]: _gone, ...spaces } = s.spaces;
                  const workspaces = s.workspaces.filter((x) => x.id !== id);
                  return { spaces, workspaces, activeWorkspaceId: s.activeWorkspaceId === id ? (workspaces[0]?.id ?? null) : s.activeWorkspaceId };
                }),
              )
              .catch(fail);
          },
        },
      });
    },

    renameWorkspaceWithPrompt: (id) => {
      const w = get().workspaces.find((x) => x.id === id);
      if (!w) return;
      set({
        dialog: {
          kind: "prompt",
          title: "Rename workspace",
          initial: w.name,
          onSubmit: (name) =>
            workspaceApi
              .rename(id, name)
              .then((u) => set((s) => ({ workspaces: s.workspaces.map((x) => (x.id === id ? u : x)) })))
              .catch(fail),
        },
      });
    },

    newTab: () => {
      const s = get();
      const ws = s.workspaces.find((w) => w.id === s.activeWorkspaceId);
      if (!ws) return;
      if (!ws.available) return set({ notice: `The folder for “${ws.name}” is missing: ${ws.rootPath}` });
      const paneId = uid("p");
      const tabId = uid("t");
      const title = uniqueTitle(tildePath(ws.rootPath), (s.spaces[ws.id]?.tabs ?? []).map((t) => t.title));
      set({ view: "terminal" });
      patchSpace(ws.id, (sp) => ({ tabs: [...sp.tabs, { id: tabId, title, layout: leaf(paneId), activePaneId: paneId }], activeTabId: tabId }));
    },

    splitActive: (dir) => {
      const tab = activeTab(get());
      if (tab) get().splitPaneAt(tab.activePaneId, dir, "after");
    },

    /** Split any pane (not only the active one), placing the new shell before or after it. */
    splitPaneAt: (paneId, dir, placement) => {
      const found = locatePane(get(), paneId);
      if (!found) return;
      const newPane = uid("p");
      patchSpace(found.wsId, (sp) => ({
        ...sp,
        tabs: sp.tabs.map((t) => (t.id === found.tab.id ? { ...t, layout: splitPane(t.layout, paneId, dir, newPane, uid("s"), placement), activePaneId: newPane } : t)),
      }));
    },

    find: null,
    openFind: (paneId) => set((s) => ({ find: { paneId, nonce: s.find?.paneId === paneId ? s.find.nonce + 1 : 0 } })),
    closeFind: () => set({ find: null }),

    contextMenu: null,
    openContextMenu: (contextMenu) => set({ contextMenu }),

    closeActivePane: () => {
      const tab = activeTab(get());
      if (tab) get().closePane(tab.activePaneId);
    },

    closePane: (paneId) => {
      const found = locatePane(get(), paneId);
      if (!found) return;
      const apply = () => {
        const { wsId, tab } = found;
        endSession(paneId);
        patchSpace(wsId, (sp) => {
          const layout = removePane(tab.layout, paneId);
          if (layout === null) {
            const idx = sp.tabs.findIndex((t) => t.id === tab.id);
            const tabs = sp.tabs.filter((t) => t.id !== tab.id);
            const next = sp.activeTabId === tab.id ? (tabs[Math.min(idx, tabs.length - 1)]?.id ?? null) : sp.activeTabId;
            return { tabs, activeTabId: next };
          }
          const activePaneId = tab.activePaneId === paneId ? paneIds(layout)[0] : tab.activePaneId;
          return { ...sp, tabs: sp.tabs.map((t) => (t.id === tab.id ? { ...t, layout, activePaneId } : t)) };
        });
        set((s) => {
          const { [paneId]: _a, ...paneState } = s.paneState;
          const { [paneId]: _b, ...paneGen } = s.paneGen;
          const { [paneId]: _c, ...paneKinds } = s.paneKinds;
          return { paneState, paneGen, paneKinds };
        });
      };
      // A pane whose process already exited needs no confirmation.
      const st = get().paneState[paneId];
      if (st && (st.kind === "exited" || st.kind === "failed")) return apply();
      const isAgent = get().paneKinds[paneId]?.type === "agent";
      void confirmIfBusy([paneId], isAgent ? "Stop this agent?" : "Close this pane?", isAgent ? "The agent process is still running. Closing will stop it." : "A command is still running in it. Closing will stop it.", apply);
    },

    closeTab: (tabId) => {
      const s = get();
      for (const [wsId, sp] of Object.entries(s.spaces)) {
        const tab = sp.tabs.find((t) => t.id === tabId);
        if (!tab) continue;
        const panes = paneIds(tab.layout);
        const apply = () => {
          panes.forEach(endSession);
          patchSpace(wsId, (cur) => {
            const idx = cur.tabs.findIndex((t) => t.id === tabId);
            const tabs = cur.tabs.filter((t) => t.id !== tabId);
            return { tabs, activeTabId: cur.activeTabId === tabId ? (tabs[Math.min(idx, tabs.length - 1)]?.id ?? null) : cur.activeTabId };
          });
        };
        void confirmIfBusy(panes, `Close “${tab.title}”?`, "A command is still running in this tab. Closing will stop it.", apply);
        return;
      }
    },

    selectTab: (tabId) => {
      const wsId = get().activeWorkspaceId;
      set({ view: "terminal" });
      if (wsId) patchSpace(wsId, (sp) => (sp.tabs.some((t) => t.id === tabId) ? { ...sp, activeTabId: tabId } : sp));
    },
    selectTabIndex: (i) => {
      const sp = activeSpace(get());
      if (sp?.tabs[i]) get().selectTab(sp.tabs[i].id);
    },
    stepTab: (delta) => {
      const sp = activeSpace(get());
      if (!sp || sp.tabs.length === 0) return;
      const i = sp.tabs.findIndex((t) => t.id === sp.activeTabId);
      get().selectTab(sp.tabs[(i + delta + sp.tabs.length) % sp.tabs.length].id);
    },
    stepPane: (delta) => {
      const tab = activeTab(get());
      if (tab) get().focusPane(cyclePane(tab.layout, tab.activePaneId, delta));
    },
    focusPane: (paneId) => {
      const found = locatePane(get(), paneId);
      if (!found) return;
      patchSpace(found.wsId, (sp) => ({ ...sp, tabs: sp.tabs.map((t) => (t.id === found.tab.id ? { ...t, activePaneId: paneId } : t)) }));
    },

    renameTabWithPrompt: (tabId) => {
      const s = get();
      const tab = tabId ? activeSpace(s)?.tabs.find((t) => t.id === tabId) : activeTab(s);
      const wsId = s.activeWorkspaceId;
      if (!tab || !wsId) return;
      set({
        dialog: {
          kind: "prompt",
          title: "Rename tab",
          initial: tab.title,
          onSubmit: (title) => {
            const t = title.trim().slice(0, 100);
            if (t) patchSpace(wsId, (sp) => ({ ...sp, tabs: sp.tabs.map((x) => (x.id === tab.id ? { ...x, title: t } : x)) }));
          },
        },
      });
    },

    setRatio: (tabId, splitId, ratio) => {
      const wsId = get().activeWorkspaceId;
      if (wsId) patchSpace(wsId, (sp) => ({ ...sp, tabs: sp.tabs.map((t) => (t.id === tabId ? { ...t, layout: setRatio(t.layout, splitId, ratio) } : t)) }));
    },

    setPaneState: (paneId, st) => {
      set((s) => ({ paneState: { ...s.paneState, [paneId]: st } }));
      // An ended process has no status to show, even if the lossy `exited` event never arrived.
      if (st.kind === "exited" || st.kind === "failed") {
        const sid = sessionRegistry.get(paneId) ?? (get().paneKinds[paneId] as { sessionId?: number } | undefined)?.sessionId;
        if (sid !== undefined) get().setSessionStatus(sid, null);
      }
      // A process just ended: it may have changed files.
      if (st.kind === "exited") void get().refreshGit();
      // A clean `exit` closes a shell pane, like other terminals. Failures stay visible, and
      // agent panes always stay so their final output can be read.
      const isAgent = get().paneKinds[paneId]?.type === "agent";
      if (get().settings.general.closeOnCleanExit && !isAgent && st.kind === "exited" && st.code === 0 && st.signal === null) get().closePane(paneId);
    },
    restartPane: (paneId) => {
      // The old process is over; forget its session so the new one starts clean.
      endSession(paneId);
      set((s) => ({ paneGen: { ...s.paneGen, [paneId]: (s.paneGen[paneId] ?? 0) + 1 } }));
    },

    refreshAgents: async () => {
      try {
        set({ agents: await agentApi.list() });
      } catch (e) {
        fail(e);
      }
    },
    setAgents: (agents) => set({ agents }),
    openLauncher: (launcherOpen) => {
      set({ launcherOpen });
      if (launcherOpen) void get().refreshAgents();
    },

    launchAgent: (agentId, opts) => {
      const s = get();
      const ws = s.workspaces.find((w) => w.id === s.activeWorkspaceId);
      const agent = s.agents.find((a) => a.id === agentId);
      if (!ws || !agent) return;
      if (!ws.available) return set({ notice: `The folder for “${ws.name}” is missing: ${ws.rootPath}` });
      const paneId = uid("p");
      const tabId = uid("t");
      set((st) => ({ view: "terminal", paneKinds: { ...st.paneKinds, [paneId]: { type: "agent", agentId, name: agent.name, worktree: opts?.worktree } } }));
      patchSpace(ws.id, (sp) => ({ tabs: [...sp.tabs, { id: tabId, title: agent.name, layout: leaf(paneId), activePaneId: paneId }], activeTabId: tabId }));
    },

    /** Bring a pane (possibly in another tab) to the front. */
    revealPane: (paneId) => {
      const found = locatePane(get(), paneId);
      if (!found) return;
      set({ view: "terminal" });
      patchSpace(found.wsId, (sp) => ({ ...sp, activeTabId: found.tab.id, tabs: sp.tabs.map((t) => (t.id === found.tab.id ? { ...t, activePaneId: paneId } : t)) }));
    },

    updateSettings: (mutate) => {
      // Serialized and applied only after the backend accepted it: overlapping edits cannot overwrite
      // each other, and an invalid value never reaches a running terminal.
      settingsQueue = settingsQueue
        .then(async () => {
          const next = mutate(structuredClone(get().settings));
          set({ settings: await settingsApi.update(next) });
          get().syncTheme();
        })
        .catch((e) => set({ notice: String(e) }));
      return settingsQueue;
    },
    resetSettings: async () => {
      try {
        set({ settings: await settingsApi.reset() });
        get().syncTheme();
      } catch (e) {
        fail(e);
      }
    },

    setFontFamilyWithPrompt: () =>
      set({
        dialog: {
          kind: "prompt",
          title: "Terminal font (e.g. Dank Mono). Type “default” to reset",
          initial: get().settings.terminal.fontFamily ?? "",
          onSubmit: (v) => {
            const reset = v.trim().toLowerCase() === "default";
            void get().updateSettings((s) => ({ ...s, terminal: { ...s.terminal, fontFamily: reset ? null : v.trim() } }));
          },
        },
      }),
    setFontSizeWithPrompt: () =>
      set({
        dialog: {
          kind: "prompt",
          title: "Terminal font size (8–32). Type “default” to reset",
          initial: String(get().settings.terminal.fontSize),
          onSubmit: (v) => {
            const reset = v.trim().toLowerCase() === "default";
            const n = Number(v.trim());
            if (!reset && !Number.isInteger(n)) return fail("Font size must be a whole number.");
            void get().updateSettings((s) => ({ ...s, terminal: { ...s.terminal, fontSize: reset ? DEFAULT_SETTINGS.terminal.fontSize : n } }));
          },
        },
      }),

    setPalette: (paletteOpen) => set({ paletteOpen }),
    setDialog: (dialog) => set({ dialog }),
    setNotice: (notice) => set({ notice }),
  };
});

export { activeSpace, activeTab };
