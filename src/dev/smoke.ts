import { invoke } from "@tauri-apps/api/core";
import { useStore, activeSpace } from "../state/store";
import { computeGeometry, paneIds } from "../features/layout/layout";
import { sessionRegistry } from "../features/terminal/registry";
import { terminalApi } from "../features/terminal/api";
import { agentApi } from "../features/agents/api";
import { workspaceApi } from "../features/workspaces/api";
import { mcpApi } from "../features/mcp/api";
import { dumpDevTerminals, getDevTerminal } from "./devTerminals";
import { settingsApi } from "../features/settings/api";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Dev-only end-to-end scenario, enabled by NAURIX_SMOKE_OUT / NAURIX_SMOKE_WORKSPACE (see README). */
let started = false;

export async function runSmoke() {
  if (started) return; // React StrictMode runs effects twice in dev
  started = true;
  const cfg = await invoke<{ out: string; workspace: string | null } | null>("debug_smoke", { text: null });
  if (!cfg) return;
  const st = () => useStore.getState();
  const report = (extra: Record<string, unknown>) => {
    const s = st();
    const summary = Object.fromEntries(
      Object.entries(s.spaces).map(([ws, sp]) => [ws, { active: sp.activeTabId, tabs: sp.tabs.map((t) => ({ title: t.title, panes: JSON.stringify(t.layout).match(/"paneId"/g)?.length })) }]),
    );
    return invoke("debug_smoke", { text: JSON.stringify({ workspaces: s.workspaces.map((w) => ({ name: w.name, available: w.available })), summary, ...extra }, null, 2) });
  };
  const type = async (paneId: string, text: string) => {
    const id = sessionRegistry.get(paneId);
    if (id !== undefined) await terminalApi.write(id, text);
  };

  try {
    const visual = await invoke<string | null>("debug_smoke_visual");
    if (visual) {
      // Visual check: one terminal in the workspace, optionally with the user's font, then wait.
      const w = await workspaceApi.create(cfg.workspace!);
      await st().init();
      st().selectWorkspace(w.id);
      if (visual === "mcp") {
        // Whole MCP flow through the real UI: add, approve, store a secret in the Keychain, test, sync, undo.
        const root = cfg.workspace!;
        st().setView("mcp");
        await sleep(800);
        await mcpApi.save({ name: "mockmcp", transport: "stdio", command: `${root}/mock-mcp-server`, args: [], url: null, env: [{ name: "API_TOKEN", value: "${secret:API_TOKEN}" }], headers: [], secrets: ["API_TOKEN"] });
        await sleep(300);
        // re-open the screen so it lists the new server
        st().setView("home");
        await sleep(200);
        st().setView("mcp");
        await sleep(1200);
        const card = () => document.querySelector('[data-mcp-server="mockmcp"]') as HTMLElement | null;
        const btn = (scope: ParentNode | null, text: string) => [...(scope?.querySelectorAll("button") ?? [])].find((b) => b.textContent?.startsWith(text)) as HTMLElement | undefined;
        const out: Record<string, unknown> = { initialCard: card()?.textContent ?? null };
        btn(card(), "Review")?.click();
        await sleep(600);
        out.approvalText = document.querySelector('[data-approval-command]')?.textContent ?? null;
        btn(document.querySelector('[role="dialog"][aria-label^="Review"]'), "Approve")?.click();
        await sleep(800);
        out.afterApprove = card()?.textContent ?? null;
        // set the secret through the UI (password input), value is the Keychain's business from here on
        btn(card(), "Set value")?.click();
        await sleep(300);
        const input = card()?.querySelector('input[type="password"]') as HTMLInputElement | null;
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "S3CRET-VALUE-9f2");
        input?.dispatchEvent(new Event("input", { bubbles: true }));
        await sleep(200);
        btn(card(), "Save")?.click();
        await sleep(900);
        out.afterSecret = card()?.textContent ?? null;
        btn(card(), "Test connection")?.click();
        await sleep(3000);
        out.testResult = (document.querySelector('[data-mcp-result="mockmcp"]') as HTMLElement | null)?.textContent ?? null;
        // sync preview + apply for Claude Code
        const claudeRow = [...document.querySelectorAll('section[aria-label="Sync to agents"] li')].find((li) => li.textContent?.includes("Claude Code"));
        btn(claudeRow ?? null, "Preview")?.click();
        await sleep(1200);
        out.planText = (document.querySelector("[data-sync-plan]") as HTMLElement | null)?.textContent ?? null;
        btn(document.querySelector("[data-sync-plan]"), "Apply")?.click();
        await sleep(500);
        btn(document.querySelector('[role="dialog"][aria-modal="true"]'), "Apply")?.click();
        await sleep(1500);
        out.afterApply = (document.querySelector('section[aria-label="Sync to agents"]') as HTMLElement | null)?.textContent ?? null;
        await report({ stage: "applied", out });
        // Window for the shell to inspect the written file and run the real claude CLI against it.
        await sleep(45000);
        const row2 = [...document.querySelectorAll('section[aria-label="Sync to agents"] li')].find((li) => li.textContent?.includes("Claude Code"));
        btn(row2 ?? null, "Undo")?.click();
        await sleep(1500);
        out.afterUndo = (document.querySelector('section[aria-label="Sync to agents"]') as HTMLElement | null)?.textContent ?? null;
        // clean up: delete the server (removes its Keychain item)
        btn(card(), "Delete")?.click();
        await sleep(400);
        btn(document.querySelector('[role="dialog"][aria-modal="true"]'), "Delete")?.click();
        await sleep(1000);
        out.afterDelete = document.querySelector('[data-mcp-server="mockmcp"]') ? "still listed" : "gone";
        await report({ stage: "undone", out });
        return;
      }
      if (visual === "skills") {
        const out: Record<string, unknown> = {};
        const until = async (fn: () => unknown, ms = 90000) => {
          const end = Date.now() + ms;
          while (Date.now() < end) {
            if (fn()) return true;
            await sleep(300);
          }
          return false;
        };
        const dlg = () => document.querySelector('[role="dialog"][aria-modal="true"]') as HTMLElement | null;
        const btn = (scope: ParentNode | null | undefined, text: string) => [...(scope?.querySelectorAll("button") ?? [])].find((b) => b.textContent?.trim().startsWith(text)) as HTMLElement | undefined;
        const setInput = (el: HTMLInputElement | null, v: string) => {
          Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(el, v);
          el?.dispatchEvent(new Event("input", { bubbles: true }));
        };
        const ws = await workspaceApi.create(cfg.workspace!);
        await st().init();
        st().selectWorkspace(ws.id);
        st().setView("skills");
        await sleep(1200);

        // 1) a REAL public repository over https: inspect only, then cancel (staging is discarded)
        btn(document.body, "Install skills")?.click();
        await sleep(500);
        setInput(dlg()?.querySelector("input") as HTMLInputElement, "https://github.com/anthropics/skills");
        await sleep(200);
        btn(dlg(), "Inspect")?.click();
        const ok = await until(() => dlg()?.textContent?.includes("Review before installing") || dlg()?.textContent?.match(/ failed| not |Only https|Could not/i));
        const cands = [...(dlg()?.querySelectorAll("[data-skill-candidate]") ?? [])] as HTMLElement[];
        out.git = { reachedReview: ok && !!dlg()?.textContent?.includes("Review before installing"), count: cands.length, names: cands.slice(0, 40).map((c) => c.getAttribute("data-skill-candidate")), header: dlg()?.textContent?.slice(0, 260), sample: cands[0]?.textContent?.slice(0, 500), flagged: cands.filter((c) => /script|command|executable/i.test(c.textContent ?? "")).length };
        btn(dlg(), "Cancel")?.click();
        await sleep(600);

        // 2) a local folder with a clean skill and a risky one
        btn(document.body, "Install skills")?.click();
        await sleep(500);
        btn(dlg(), "Folder on this Mac")?.click();
        await sleep(200);
        setInput(dlg()?.querySelector("input") as HTMLInputElement, `${cfg.workspace}/../skills-src`);
        await sleep(200);
        btn(dlg(), "Inspect")?.click();
        await until(() => dlg()?.textContent?.includes("Review before installing"), 15000);
        out.local = (dlg() as HTMLElement | null)?.textContent?.slice(0, 1400) ?? null;
        for (const cb of [...(dlg()?.querySelectorAll('input[type="checkbox"]') ?? [])] as HTMLInputElement[]) if (!cb.checked) cb.click();
        await sleep(300);
        btn(dlg(), "Install")?.click();
        await until(() => !dlg(), 15000);
        await sleep(1000);
        out.listAfterInstall = [...document.querySelectorAll("[data-skill]")].map((c) => c.textContent?.slice(0, 260));

        // 3) enable in the project folders of three agents
        const card = () => document.querySelector('[data-skill="reviewer"]') as HTMLElement | null;
        btn(card(), "Use with agents")?.click();
        await sleep(600);
        const rows = () => [...(dlg()?.querySelectorAll("tbody tr") ?? [])] as HTMLElement[];
        for (const label of ["Claude Code", "Codex CLI", "OpenCode"]) {
          const cb = rows().find((r) => r.textContent?.startsWith(label))?.querySelectorAll('input[type="checkbox"]')[1] as HTMLInputElement | undefined;
          cb?.click();
          await sleep(500);
        }
        out.matrix = rows().map((r) => r.textContent?.replace(/\s+/g, " ").slice(0, 200));
        btn(dlg(), "Done")?.click();
        await sleep(600);
        out.cardAfterLink = card()?.textContent?.slice(0, 400) ?? null;
        btn(card(), "Check for updates")?.click();
        await sleep(2500);
        out.updateInfo = document.body.textContent?.includes("is up to date") ?? false;
        await report({ stage: "linked", out });

        await sleep(40000); // window for the shell to inspect the links on disk

        for (const name of ["reviewer", "risky"]) {
          const c = document.querySelector(`[data-skill="${name}"]`) as HTMLElement | null;
          btn(c, "Uninstall")?.click();
          await sleep(500);
          btn(document.querySelector('[role="dialog"][aria-modal="true"]'), "Uninstall")?.click();
          await sleep(1200);
        }
        out.afterUninstall = [...document.querySelectorAll("[data-skill]")].length;
        await report({ stage: "uninstalled", out });
        return;
      }
      if (visual === "git") {
        const out: Record<string, unknown> = {};
        const until = async (fn: () => unknown, ms = 20000) => {
          const end = Date.now() + ms;
          while (Date.now() < end) {
            if (fn()) return true;
            await sleep(250);
          }
          return false;
        };
        const btn = (scope: ParentNode | null | undefined, text: string) => [...(scope?.querySelectorAll("button") ?? [])].find((b) => b.textContent?.trim().startsWith(text)) as HTMLElement | undefined;
        const view = () => document.querySelector("[data-git-header]")?.textContent ?? null;
        const root = cfg.workspace!; // a subfolder of the repository
        const sub = await workspaceApi.create(root);
        const evil = await workspaceApi.create(`${root}/../../evil`);
        const plain = await workspaceApi.create(`${root}/../../plain`);
        await st().init();

        // 1) workspace inside a repository (subfolder): status reports the repository root
        st().selectWorkspace(sub.id);
        st().setView("git");
        await until(() => view());
        out.header = view();
        out.sections = [...document.querySelectorAll("[data-git-section]")].map((e) => (e as HTMLElement).textContent?.replace(/\s+/g, " ").slice(0, 300));
        const clickFile = async (name: string, section?: string) => {
          const scope = section ? document.querySelector(`[data-git-section="${section}"]`) : document;
          const b = [...(scope?.querySelectorAll("button") ?? [])].find((x) => x.textContent?.includes(name)) as HTMLElement | undefined;
          b?.click();
          await sleep(1500);
          return (document.querySelector("[data-git-diff]") as HTMLElement | null)?.textContent?.slice(0, 500) ?? null;
        };
        out.diffUnstaged = await clickFile("a.txt", "Changes");
        out.diffStaged = await clickFile("b.txt", "Staged");
        out.diffUntracked = await clickFile("u.txt", "Untracked");
        out.diffRename = await clickFile("c2.txt", "Staged");
        out.statusBarBranch = document.querySelector("[data-status-branch]")?.textContent ?? null;
        await report({ stage: "git1", out });

        // window for the shell: screenshot + change the repository from outside
        await sleep(25000);
        window.dispatchEvent(new Event("focus")); // what happens when you come back to the window
        await sleep(2500);
        out.afterExternalChange = view();
        out.sectionsAfter = [...document.querySelectorAll("[data-git-section]")].map((e) => (e as HTMLElement).textContent?.replace(/\s+/g, " ").slice(0, 200));

        // 2) "Open in terminal" starts at the repository root, not at the workspace subfolder
        btn(document.body, "Open in terminal")?.click();
        await sleep(2500);
        const pane = Object.keys(dumpDevTerminals()).pop()!;
        const id = sessionRegistry.get(pane);
        if (id !== undefined) await terminalApi.write(id, "echo CWD=$(pwd)END\r");
        await sleep(1500);
        // wrapped lines are joined so a long path can be read back whole
        out.terminalPwd = ((dumpDevTerminals()[pane] ?? []).join("").match(/CWD=(.*?)END/g) ?? []).pop() ?? null;
        out.workspaceDir = root;
        st().setView("git");

        // 3) a repository whose own config defines a filter: refused until trusted
        st().selectWorkspace(evil.id);
        await until(() => document.body.textContent?.includes("runs commands from its own configuration"));
        out.blocked = document.body.textContent?.includes("runs commands from its own configuration") ?? false;
        out.blockedKeys = [...document.querySelectorAll("span.font-mono")].map((e) => e.textContent).filter((t) => t?.includes("filter.")).join();
        await report({ stage: "git2", out });

        // 4) a folder that is not a repository
        st().selectWorkspace(plain.id);
        await until(() => document.body.textContent?.includes("not a Git repository"));
        out.notRepo = document.body.textContent?.includes("not a Git repository") ?? false;
        await report({ stage: "git3", out });
        return;
      }
      if (visual === "themes") {
        // Every theme through the real UI: pick it in Settings -> Appearance, check CSS variables and the live
        // xterm theme, check the shells/scrollback survived, and dwell so the window can be screenshotted.
        const { THEMES } = await import("../themes/registry");
        const ws = await workspaceApi.create(cfg.workspace!);
        await st().init();
        st().selectWorkspace(ws.id);
        st().newTab();
        await sleep(2500);
        st().splitActive("row");
        await sleep(2000);
        const panes = Object.keys(dumpDevTerminals());
        for (const p of panes) await type(p, `echo MARK-${p.slice(-4)}; printf '\\033[31mred\\033[32m green\\033[33m yellow\\033[34m blue\\033[35m mag\\033[36m cyan\\033[0m plain\\n'; ls\r`);
        await sleep(1500);
        const idsBefore = panes.map((p) => sessionRegistry.get(p));
        const results: Record<string, unknown>[] = [];
        const sectionBtn = (name: string) => [...document.querySelectorAll('nav[aria-label="Settings sections"] button')].find((b) => b.textContent === name) as HTMLElement | undefined;
        for (const t of THEMES) {
          st().setView("settings");
          await sleep(400);
          sectionBtn("Appearance")?.click();
          await sleep(400);
          (document.querySelector(`[data-theme-card="${t.id}"]`) as HTMLElement | null)?.click();
          await sleep(900);
          const root = document.documentElement;
          const cs = getComputedStyle(root);
          const term = getDevTerminal(panes[0])!;
          const row = {
            id: t.id,
            dataTheme: root.dataset.theme,
            bgVar: cs.getPropertyValue("--color-midnight").trim(),
            expectedBg: t.tokens.background,
            bodyBg: getComputedStyle(document.body).backgroundColor,
            xtermBg: term.options.theme?.background,
            expectedTermBg: t.tokens.terminal.background,
            xtermRed: term.options.theme?.red,
            colorScheme: root.style.colorScheme,
            persisted: (await settingsApi.get()).appearance,
            sameSessions: JSON.stringify(panes.map((p) => sessionRegistry.get(p))) === JSON.stringify(idsBefore),
            markers: panes.map((p) => (dumpDevTerminals()[p] ?? []).some((l) => l.includes(`MARK-${p.slice(-4)}`))),
          };
          results.push(row);
          await report({ stage: "theme-settings", id: t.id, results });
          await sleep(3500);
          st().setView("terminal");
          await sleep(900);
          await report({ stage: "theme-terminal", id: t.id, results });
          await sleep(3500);
        }
        // accent + palette + system mode
        st().setView("settings");
        await sleep(400);
        sectionBtn("Appearance")?.click();
        await sleep(300);
        await report({ stage: "done", results, panes: panes.length });
        return;
      }
      if (visual === "openws") {
        // Picking a project with no terminal yet opens one (sidebar path), and does not when a tool screen is showing.
        const a = await workspaceApi.create(cfg.workspace!);
        await st().init();
        const out: Record<string, unknown> = {};
        st().openWorkspace(a.id);
        await sleep(2500);
        out.afterFirstPick = { view: st().view, tabs: st().spaces[a.id]?.tabs.length, sessions: Object.keys(dumpDevTerminals()).length };
        st().openWorkspace(a.id);
        await sleep(500);
        out.pickAgain = { tabs: st().spaces[a.id]?.tabs.length };
        await report({ stage: "openws", out });
        return;
      }
      if (visual === "settings2") {
        // second run on the same data folder: do the settings survive a restart?
        await report({ stage: "settings2", fromBackend: await settingsApi.get() });
        return;
      }
      if (visual === "settings") {
        const out: Record<string, unknown> = {};
        const until = async (fn: () => unknown, ms = 15000) => {
          const end = Date.now() + ms;
          while (Date.now() < end) {
            if (fn()) return true;
            await sleep(200);
          }
          return false;
        };
        const set = (el: HTMLInputElement | HTMLSelectElement | null, v: string) => {
          const proto = el instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
          Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(el, v);
          el?.dispatchEvent(new Event(el instanceof HTMLSelectElement ? "change" : "input", { bubbles: true }));
        };
        const enter = async (label: string, v: string) => {
          const el = document.querySelector(`[aria-label="${label}"]`) as HTMLInputElement | null;
          set(el, v);
          el?.focus();
          el?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
          el?.blur();
          await sleep(500);
        };
        const sectionBtn = (name: string) => [...document.querySelectorAll('nav[aria-label="Settings sections"] button')].find((b) => b.textContent === name) as HTMLElement | undefined;
        const arg0 = async (paneId: string) => {
          const id = sessionRegistry.get(paneId);
          if (id !== undefined) await terminalApi.write(id, "shopt -q login_shell && echo LOGINYES || echo LOGINNO\r");
          await sleep(1500);
          return (dumpDevTerminals()[paneId] ?? []).map((l) => l.trim()).filter((l) => l === "LOGINYES" || l === "LOGINNO").pop() ?? null;
        };
        const ws = await workspaceApi.create(cfg.workspace!);
        await st().init();
        st().selectWorkspace(ws.id);
        st().newTab();
        await sleep(2500);
        const pane1 = Object.keys(dumpDevTerminals())[0];
        const term1 = () => getDevTerminal(pane1)!;
        out.arg0Login = await arg0(pane1);
        out.defaultsBackend = await settingsApi.get();

        st().setView("settings");
        await sleep(800);
        const sections: Record<string, string> = {};
        for (const name of ["General", "Terminal", "AI Agents", "MCP Integrations", "Agent Skills", "Performance", "Security"]) {
          sectionBtn(name)?.click();
          await sleep(900);
          sections[name] = (document.querySelector("[data-settings-section]") as HTMLElement | null)?.textContent?.replace(/\s+/g, " ").slice(0, 900) ?? "";
        }
        out.sections = sections;

        // Terminal section: values apply LIVE to the running terminal
        sectionBtn("Terminal")?.click();
        await sleep(500);
        await enter("Font size", "16");
        await enter("Scrollback lines", "1500");
        set(document.querySelector('[aria-label="Cursor style"]') as HTMLSelectElement, "bar");
        await sleep(400);
        (document.querySelector('input[type="checkbox"][aria-label="Start shells as login shells"]') as HTMLInputElement | null)?.click();
        await sleep(400);
        out.liveTerminal = { fontSize: term1().options.fontSize, scrollback: term1().options.scrollback, cursorStyle: term1().options.cursorStyle };

        // invalid value: rejected by the backend, UI keeps the old value and says why
        await enter("Scrollback lines", "100");
        out.afterInvalid = { scrollback: term1().options.scrollback, stored: (await settingsApi.get()).terminal.scrollback, notice: st().notice };
        st().setNotice(null);

        // renderer + blink + measure
        sectionBtn("Performance")?.click();
        await sleep(500);
        set(document.querySelector('[aria-label="Terminal renderer"]') as HTMLSelectElement, "webgl");
        await sleep(300);
        (document.querySelector('input[type="checkbox"][aria-label="Blink the cursor"]') as HTMLInputElement | null)?.click();
        await sleep(300);
        [...document.querySelectorAll("button")].find((b) => b.textContent === "Measure")?.click();
        await until(() => document.querySelector("[data-perf]"));
        out.perf = document.querySelector("[data-perf]")?.textContent ?? null;
        st().setView("terminal");
        await sleep(1500);
        const screen = term1().element?.querySelector(".xterm-screen");
        out.webgl = { canvases: screen?.querySelectorAll("canvas").length ?? -1, blink: term1().options.cursorBlink, status: document.querySelector("[data-renderer-status]")?.textContent ?? st().rendererStatus };

        // a NEW terminal now starts as a non-login shell
        st().newTab();
        await sleep(2500);
        const pane2 = Object.keys(dumpDevTerminals()).find((p) => p !== pane1)!;
        out.arg0NonLogin = await arg0(pane2);
        out.final = (await settingsApi.get()).terminal;
        st().setView("settings");
        sectionBtn("Security")?.click();
        await sleep(1200);
        out.security = (document.querySelector("[data-settings-section]") as HTMLElement | null)?.textContent?.replace(/\s+/g, " ").slice(0, 900);
        await report({ stage: "settings", out });
        return;
      }
      if (visual === "adapters") {
        // Read what the launcher shows for each agent (list + review step), without launching anything.
        st().openLauncher(true);
        await sleep(1500);
        const dialog = () => document.querySelector('[role="dialog"][aria-label="Launch AI agent"]');
        const listText = dialog()?.textContent ?? null;
        const review: Record<string, string | null> = {};
        for (const name of ["Gemini CLI", "Claude Code", "Antigravity CLI"]) {
          const row = [...(dialog()?.querySelectorAll("li") ?? [])].find((li) => li.textContent?.includes(name));
          const btn = [...(row?.querySelectorAll("button") ?? [])].find((b) => /Launch|Set up/.test(b.textContent ?? "")) as HTMLElement | undefined;
          btn?.click();
          await sleep(700);
          review[name] = dialog()?.textContent ?? null;
          ([...(dialog()?.querySelectorAll("button") ?? [])].find((b) => b.textContent === "Back") as HTMLElement | undefined)?.click();
          await sleep(300);
        }
        await report({ stage: "adapters", listText, review });
        return;
      }
      if (visual === "menu") {
        // Exercise the right-click menu through real DOM events, in all four split directions.
        st().newTab();
        await sleep(2000);
        const tab0 = () => activeSpace(st())!.tabs.find((t) => t.id === activeSpace(st())!.activeTabId)!;
        const origin = tab0().activePaneId;
        const rightClick = async (paneId: string, label?: string) => {
          const el = document.querySelector(`[data-pane="${paneId}"]`) as HTMLElement;
          const r = el.getBoundingClientRect();
          el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }));
          await sleep(300);
          if (!label) return;
          const item = [...document.querySelectorAll('[role="menuitem"]')].find((b) => b.textContent?.startsWith(label)) as HTMLElement | undefined;
          item?.click();
          await sleep(500);
        };
        const items = async () => (await rightClick(origin), [...document.querySelectorAll('[role="menuitem"]')].map((b) => ({ text: b.textContent, disabled: (b as HTMLButtonElement).disabled })));
        const menuItems = await items();
        useStore.getState().openContextMenu(null);
        await sleep(200);
        const pre = new Set(paneIds(tab0().layout));
        const newPane = () => paneIds(tab0().layout).find((p) => !pre.has(p))!;
        const rects: Record<string, unknown> = {};
        for (const label of ["Split Left", "Split Up", "Split Right", "Split Down"]) {
          await rightClick(origin, label);
          const np = newPane();
          pre.add(np);
          const g = computeGeometry(tab0().layout).panes;
          rects[label] = { newPane: g[np], origin: g[origin] };
        }
        const g = computeGeometry(tab0().layout).panes;
        const o = g[origin];
        const side = (label: string) => (rects[label] as { newPane: { x: number; y: number; w: number; h: number } }).newPane;
        const checks = {
          panes: Object.keys(g).length,
          leftIsLeftOfOriginAtTheTime: side("Split Left").x < (rects["Split Left"] as { origin: { x: number } }).origin.x,
          upIsAbove: side("Split Up").y < (rects["Split Up"] as { origin: { y: number } }).origin.y,
          rightIsRight: side("Split Right").x > (rects["Split Right"] as { origin: { x: number } }).origin.x,
          downIsBelow: side("Split Down").y > (rects["Split Down"] as { origin: { y: number } }).origin.y,
          originStillMounted: !!document.querySelector(`[data-pane="${origin}"]`),
          originRect: o,
        };
        await rightClick(origin); // leave the menu open for a screenshot
        await report({ stage: "menu", menuItems, checks, rects });
        return;
      }
      if (visual === "terminal") st().newTab();
      await sleep(1000);
      const checks = { meslo: document.fonts.check('13px "MesloLGS NF"'), dank: document.fonts.check('13px "Dank Mono"'), settings: st().settings.terminal };
      await report({ stage: "visual", checks });
      return;
    }
    if (cfg.workspace) {
      const w = await workspaceApi.create(cfg.workspace);
      await st().init();
      st().selectWorkspace(w.id);
    }
    st().newTab();
    await sleep(1500);
    const first = Object.keys(dumpDevTerminals())[0];
    await type(first, "echo MARK_BEFORE_SPLIT\r");
    await sleep(1000);
    st().splitActive("row");
    await sleep(2000);
    // Regression guard: splitting must not restart the original shell.
    const survived = dumpDevTerminals()[first]?.some((l) => l.startsWith("MARK_BEFORE_SPLIT")) ?? false;
    st().newTab();
    await sleep(800);
    st().splitActive("col");
    await sleep(1500);
    const panes = Object.keys(dumpDevTerminals());
    for (const p of panes) await type(p, `echo PANE_${p}_$((6*7)); pwd\r`);
    await sleep(2500);
    await report({ stage: "after-typing", survivedSplit: survived, buffers: dumpDevTerminals() });

    // Exit one pane cleanly: it should close itself.
    const victim = panes[panes.length - 1];
    await type(victim, "exit\r");
    await sleep(2500);
    await report({ stage: "after-exit-shell", survivedSplit: survived, closedPane: victim, buffers: dumpDevTerminals(), activeTabs: activeSpace(st())?.tabs.length });

    // ---- Agents (M5) ----
    const listed = await agentApi.list();
    const found = Object.fromEntries(listed.filter((a) => a.kind === "preset").map((a) => [a.id, a.resolvedPath]));
    const mock = (await agentApi.saveCustom({ name: "Mock Agent", executable: `${cfg.workspace}/mock-agent`, args: ["--demo"], env: [] })).find((a) => a.name === "Mock Agent")!;
    await st().refreshAgents();
    const agentPanes = () => Object.entries(st().paneKinds).filter(([, k]) => k.type === "agent").map(([id]) => id);

    st().launchAgent(mock.id, false); // not reviewed: the backend must refuse
    await sleep(2500);
    const unreviewed = { ...st().paneState[agentPanes()[0]] };

    st().launchAgent(mock.id, true);
    await sleep(2500);
    const running = agentPanes()[1];
    await type(running, "hello\r");
    await sleep(1000);
    const runningState = { ...st().paneState[running] };
    await type(running, "bye\r"); // mock exits 0: an agent pane must NOT auto-close
    await sleep(2000);
    const afterClean = { state: { ...st().paneState[running] }, stillThere: agentPanes().includes(running) };

    st().launchAgent("codex", true); // not installed on this machine
    await sleep(2500);
    const codexPane = agentPanes()[2];
    const notInstalled = { ...st().paneState[codexPane] };

    // Render the launcher and read what the user would see.
    st().openLauncher(true);
    await sleep(1000);
    const dialogText = document.querySelector('[role="dialog"][aria-label="Launch AI agent"]')?.textContent ?? null;
    const launchButtons = document.querySelectorAll('[aria-label="Launch AI agent"] button').length;
    st().openLauncher(false);
    const sidebarText = document.querySelector("aside")?.textContent ?? null;

    await report({ stage: "agents", dialogText, launchButtons, sidebarText, found, unreviewed, runningState, afterClean, notInstalled, agentPaneCount: agentPanes().length, buffers: dumpDevTerminals() });
    await agentApi.deleteCustom(mock.id);
  } catch (e) {
    await report({ stage: "error", error: String(e) });
  }
}
