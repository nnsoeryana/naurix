import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useStore, activeSpace } from "../state/store";
import { computeGeometry, paneIds } from "../features/layout/layout";
import { sessionRegistry } from "../features/terminal/registry";
import { sessionApi, terminalApi } from "../features/terminal/api";
import { agentApi } from "../features/agents/api";
import { approvalApi } from "../features/approvals/api";
import { worktreeApi } from "../features/worktrees/api";
import { workflowApi } from "../features/workflows/api";
import { memoryApi } from "../features/memory/api";
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
      if (visual === "worktree") {
        // Agent worktrees (ADR 0028) end to end, against a real Git repository prepared by the shell.
        const wsId = w.id;
        const A = (await agentApi.saveCustom({ name: "WT Agent", executable: `${cfg.workspace}/mock-agent`, args: ["--wt"], env: [] })).find((a) => a.name === "WT Agent")!;
        await st().refreshAgents();
        const out: Record<string, unknown> = {};
        const attempt = async (f: () => Promise<unknown>) => f().then(() => "ok", (e) => String(e));
        const agentPanes = () => Object.entries(st().paneKinds).filter(([, k]) => k.type === "agent").map(([id]) => id);
        const paneText = (pane: string) => (dumpDevTerminals()[pane] ?? []).join("\n");
        const waitFor = async (cond: () => boolean, ms = 10000) => {
          const end = Date.now() + ms;
          while (Date.now() < end && !cond()) await sleep(100);
          return cond();
        };
        out.support = await worktreeApi.support(wsId);
        const auth = await agentApi.authorize(wsId, A.id, true);
        out.requestClasses = auth.kind === "required" ? auth.requests.map((r) => r.class) : [];
        out.worktreeReview = auth.kind === "required" ? auth.requests.find((r) => r.class === "create_worktree")?.display : null;

        st().launchAgent(A.id, { worktree: true }); // nothing approved yet: must be refused and create nothing
        await sleep(2500);
        out.unapproved = { ...st().paneState[agentPanes()[0]] };
        out.worktreesAfterRefusal = (await worktreeApi.list(wsId)).length;

        if (auth.kind === "required") {
          for (const r of auth.requests) await approvalApi.decide(r.id, { decision: "allow", scope: r.class === "launch_agent" ? "workspace" : "once" });
        }
        st().launchAgent(A.id, { worktree: true });
        await waitFor(() => agentPanes().length === 2 && st().paneState[agentPanes()[1]]?.kind === "running", 20000);
        const apane = agentPanes()[1];
        out.agent = { ...st().paneState[apane] };
        await sleep(800);
        const made = await worktreeApi.list(wsId);
        out.created = made.map((m) => ({ branch: m.branch, path: m.path, exists: m.exists }));
        out.branchBadge = (st().paneKinds[apane] as { branch?: string }).branch ?? null;
        out.sidebarBadge = document.querySelector("[data-worktree-branch]")?.textContent ?? null;
        const wt = made[0];

        // A terminal inside the worktree (the backend resolves the folder). Leave an uncommitted file behind.
        const shellPanes = () => Object.entries(st().paneKinds).filter(([, k]) => k.type === "shell").map(([id]) => id);
        st().newWorktreeTab("wt-shell", wt.id);
        await waitFor(() => shellPanes().length >= 1 && st().paneState[shellPanes()[shellPanes().length - 1]]?.kind === "running", 15000);
        const sh1 = shellPanes()[shellPanes().length - 1];
        await sleep(600);
        await type(sh1, "pwd; touch dirty.txt; exit\r");
        await waitFor(() => st().paneState[sh1]?.kind === "exited" || !(sh1 in st().paneState), 10000);
        out.shellPwd = paneText(sh1).split("\n").filter((l) => l.startsWith("/"))[0] ?? paneText(sh1).slice(-200);

        out.removeWhileAgentRuns = await attempt(() => worktreeApi.reviewRemove(wt.id)); // an agent is still in it
        await sessionApi.close(sessionRegistry.get(apane)!);
        await sleep(1500);
        out.removeWhenDirty = await attempt(() => worktreeApi.reviewRemove(wt.id)); // uncommitted file

        st().newWorktreeTab("wt-shell2", wt.id);
        await waitFor(() => shellPanes().length >= 2 && st().paneState[shellPanes()[shellPanes().length - 1]]?.kind === "running", 15000);
        const sh2 = shellPanes()[shellPanes().length - 1];
        await sleep(600);
        await type(sh2, "rm dirty.txt; exit\r");
        await waitFor(() => st().paneState[sh2]?.kind === "exited" || !(sh2 in st().paneState), 10000);
        await sleep(800);

        const review = await worktreeApi.reviewRemove(wt.id).catch((e) => String(e));
        out.removeReview = typeof review === "string" ? review : review.display;
        if (typeof review !== "string") {
          out.removeWithoutApproval = await attempt(() => worktreeApi.remove(wt.id)); // must need the approval first
          await approvalApi.decide(review.id, { decision: "allow", scope: "once" });
          out.removed = await attempt(() => worktreeApi.remove(wt.id));
        }
        out.listAfter = (await worktreeApi.list(wsId)).length;
        out.history = (await approvalApi.list()).history.map((g) => `${g.state}:${g.scope}:${g.class}`);
        await report({ stage: "worktree", out });
        return;
      }
      if (visual === "memory") {
        // Project memory (ADR 0031) end to end through the Memory view.
        const out: Record<string, unknown> = {};
        const q = (sel: string) => document.querySelector(sel) as HTMLElement | null;
        const all = (sel: string) => [...document.querySelectorAll(sel)] as HTMLElement[];
        const waitFor = async (cond: () => boolean, ms = 10000) => {
          const end = Date.now() + ms;
          while (Date.now() < end && !cond()) await sleep(120);
          return cond();
        };
        const setValue = (el: HTMLElement, v: string) => {
          const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
          Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(el, v);
          el.dispatchEvent(new Event("input", { bubbles: true }));
        };
        const click = (label: string) => all("button").find((b) => b.textContent === label)?.click();
        const addNote = async (title: string, body: string) => {
          q("[data-memory-new]")!.click();
          await waitFor(() => !!q("[data-memory-editor]"));
          setValue(q("[aria-label='Note title']")!, title);
          setValue(q("[aria-label='Note text']")!, body);
          await sleep(200);
          q("[data-memory-save]")!.click();
          await sleep(900);
        };
        st().setView("memory");
        await sleep(900);
        out.offText = q("[data-memory-off]")?.textContent ?? null;
        out.saveWhileOff = await memoryApi.save(w.id, null, "t", "b").then(() => "saved", (e) => String(e));
        (q("[data-memory-toggle]") as HTMLInputElement).click();
        await waitFor(() => !!q("[data-memory-new]"));
        await addNote("Database choice", "We use SQLite with FTS5 for search.");
        await addNote("Naming", "Crates are named naurix-*.");
        await addNote("Oneliner note", "");
        out.afterThree = all("[data-note]").length;
        // A secret is refused, names the line and kind, and is not stored.
        const secret = "AKIAIOSFODNN7EXAMPLE";
        await addNote("Deploy", `first line\naws ${secret}`);
        out.secretError = q("[data-memory-error]")?.textContent ?? null;
        out.secretEchoed = (q("[data-memory-error]")?.textContent ?? "").includes(secret);
        click("Cancel");
        await sleep(400);
        out.countAfterSecret = (await memoryApi.status(w.id)).count;
        // Search.
        setValue(q("[aria-label='Search notes']")!, "fts");
        await sleep(900);
        out.searchFts = all("[data-note]").map((e) => e.textContent?.slice(0, 20));
        setValue(q("[aria-label='Search notes']")!, "zzzz");
        await sleep(900);
        out.searchNone = all("[data-note]").length;
        setValue(q("[aria-label='Search notes']")!, "");
        await sleep(600);

        // Insert: a running agent pane receives the text without Enter; a shell pane is refused.
        const cat = (await agentApi.saveCustom({ name: "Cat agent", executable: "/bin/sh", args: ["-c", "cat"], env: [] })).find((a) => a.name === "Cat agent")!;
        await st().refreshAgents();
        const r = await agentApi.authorize(w.id, cat.id);
        if (r.kind === "required") for (const x of r.requests) await approvalApi.decide(x.id, { decision: "allow", scope: "workspace" });
        st().launchAgent(cat.id);
        const agentPane = () => Object.entries(st().paneKinds).find(([, k]) => k.type === "agent")?.[0];
        await waitFor(() => !!agentPane() && st().paneState[agentPane()!]?.kind === "running", 15000);
        await sleep(800);
        st().setView("memory");
        await sleep(600);
        // A note with several lines is refused for a program without bracketed paste (cat); a one-line note goes in.
        const insertFor = (title: string) => (all("[data-note]").find((e) => e.textContent?.startsWith(title))?.querySelector("button") as HTMLElement | undefined)?.click();
        insertFor("Naming");
        await sleep(500);
        out.multiLineRefusal = q("[data-memory-error]")?.textContent ?? null;
        out.viewAfterRefusal = st().view;
        insertFor("Oneliner note");
        await sleep(1200);
        const buf = () => (dumpDevTerminals()[agentPane()!] ?? []).join("\n");
        out.viewAfterInsert = st().view;
        out.agentBufferHasNote = buf().includes("Oneliner note");
        out.agentBufferHasEnterEffect = buf().split("\n").filter((l) => l.includes("Oneliner note")).length;
        out.agentBuffer = buf().slice(-160);
        st().newTab(); // a plain shell becomes the active pane
        await sleep(1800);
        st().setView("memory");
        await sleep(600);
        insertFor("Oneliner note");
        await sleep(500);
        out.shellRefusal = q("[data-memory-error]")?.textContent ?? null;

        // Turning it off hides the notes but keeps them; delete works.
        (q("[data-memory-toggle]") as HTMLInputElement).click();
        await sleep(900);
        out.offAgain = q("[data-memory-off]")?.textContent ?? null;
        out.noteRowsWhileOff = all("[data-note]").length;
        out.statusOff = await memoryApi.status(w.id);
        await report({ stage: "memory", out });
        return;
      }
      if (visual === "workflow") {
        // Workflows (ADR 0030) end to end through the real Workflows view. Agents are custom `sh -c` commands.
        const out: Record<string, unknown> = {};
        const mk = async (name: string, script: string) => (await agentApi.saveCustom({ name, executable: "/bin/sh", args: ["-c", script], env: [] })).find((a) => a.name === name)!;
        const slow = await mk("Slow ok", "sleep 2; exit 0");
        const ok = await mk("Quick ok", "echo PLANTED-OUTPUT-MARKER; exit 0");
        const bad = await mk("Quick fail", "exit 3");
        const hang = await mk("Hang", "sleep 300");
        await st().refreshAgents();
        await workflowApi.save(w.id, null, "Build then check", [
          { name: "build", agentId: slow.id, dependsOn: [] },
          { name: "unit", agentId: ok.id, dependsOn: [0] },
          { name: "lint", agentId: bad.id, dependsOn: [0] },
          { name: "ship", agentId: ok.id, dependsOn: [1, 2] },
        ]);
        out.cycleRefused = await workflowApi.save(w.id, null, "cycle", [{ name: "a", agentId: ok.id, dependsOn: [] }, { name: "b", agentId: ok.id, dependsOn: [0] }].map((x, i) => (i === 0 ? { ...x, dependsOn: [1] } : x))).then(() => "saved", (e) => String(e));
        st().setView("workflows");
        await sleep(800);
        const q = (sel: string) => document.querySelector(sel) as HTMLElement | null;
        const all = (sel: string) => [...document.querySelectorAll(sel)] as HTMLElement[];
        const waitFor = async (cond: () => boolean, ms = 20000) => {
          const end = Date.now() + ms;
          while (Date.now() < end && !cond()) await sleep(150);
          return cond();
        };
        out.listed = all("[data-workflow]").map((e) => e.textContent);
        out.activeBefore = (await approvalApi.list()).active.length;
        q("[data-workflow-run]")!.click();
        await waitFor(() => !!q("[data-run-review]"));
        out.review = q("[data-run-review] pre")?.textContent ?? null;
        out.sessionsBeforeApproval = (await sessionApi.list()).length;
        q("[data-run-approve]")!.click();
        const seen: string[] = [];
        await waitFor(() => {
          const snap = all("[data-graph-step]").map((e) => `${e.querySelector("[data-graph-label]")?.textContent}`).join(" | ");
          if (seen[seen.length - 1] !== snap) seen.push(snap);
          return q("[data-run-state]")?.textContent === "failed" || q("[data-run-state]")?.textContent === "succeeded";
        }, 30000);
        out.progress = seen;
        out.graph = { nodes: all(".react-flow__node").length, edges: all(".react-flow__edge").length, positions: all(".react-flow__node").map((n) => n.style.transform) };
        out.graphStates = all("[data-graph-step]").map((e) => e.getAttribute("data-graph-step"));
        // The list stays available as the alternative view.
        q("[data-run-mode='list']")!.click();
        await sleep(500);
        out.listStepsAfterToggle = all("[data-run-step]").length;
        out.graphGoneInList = all(".react-flow__node").length;
        q("[data-run-mode='graph']")!.click();
        await sleep(800);
        out.firstRunState = q("[data-run-state]")?.textContent ?? null;
        out.grantsAfterRun = (await approvalApi.list()).active.length;
        out.history = (await approvalApi.list()).history.map((g) => `${g.state}:${g.scope}:${g.class}`);

        // Second workflow: an interactive-style step that never exits, finished by the user.
        const wf2 = await workflowApi.save(w.id, null, "Manual finish", [{ name: "agent", agentId: hang.id, dependsOn: [] }, { name: "after", agentId: ok.id, dependsOn: [0] }]);
        await st().refreshAgents();
        const start2 = await workflowApi.run(wf2.id);
        if (start2.kind === "required") await approvalApi.decide(start2.request.id, { decision: "allow", scope: "once" });
        const started2 = await workflowApi.run(wf2.id);
        await sleep(1200);
        st().bumpRuns();
        await sleep(600);
        const hangSid = started2.kind === "started" ? Object.values(started2.run.sessions)[0] : undefined;
        out.hangSession = hangSid ?? null;
        out.markDoneButtons = all("button").filter((b) => b.textContent === "Mark done").length;
        out.openTerminalButtons = all("button").filter((b) => b.textContent === "Open terminal").length;
        const panesBefore = Object.keys(st().paneKinds).length;
        all("button").find((b) => b.textContent === "Open terminal")?.click();
        await sleep(1500);
        out.openedPane = Object.keys(st().paneKinds).length - panesBefore;
        st().setView("workflows");
        await sleep(800);
        all("button").find((b) => b.textContent === "Mark done")?.click();
        await waitFor(() => all("[data-run-state]")[0]?.textContent === "succeeded", 15000);
        out.secondRunState = all("[data-run-state]")[0]?.textContent ?? null;
        out.hangStillRunning = hangSid !== undefined ? (await sessionApi.list()).some((x) => x.id === hangSid && x.running) : null;
        if (hangSid !== undefined) await sessionApi.close(hangSid);
        out.grantsAtEnd = (await approvalApi.list()).active.length;
        await report({ stage: "workflow", out });
        return;
      }
      if (visual === "status") {
        // Status signals (ADR 0029) end to end. `claude-code` is pointed at scripts/mock-status-agent, which runs
        // the hooks it was given the way Claude Code would. Off first (no hooks, plain Running), then opted in.
        await agentApi.setPath("claude-code", `${cfg.workspace}/mock-status-agent`);
        await st().refreshAgents();
        const out: Record<string, unknown> = {};
        const agentPanes = () => Object.entries(st().paneKinds).filter(([, k]) => k.type === "agent").map(([id]) => id);
        const last = (sel: string) => [...document.querySelectorAll(sel)].pop();
        const label = () => last("[data-agent-status]")?.textContent ?? null;
        const approve = async (wsId: string) => {
          const r = await agentApi.authorize(wsId, "claude-code");
          if (r.kind === "required") {
            for (const q of r.requests) await approvalApi.decide(q.id, { decision: "allow", scope: "workspace" });
          }
          return r.kind === "required" ? r.requests[0].display : null;
        };
        out.offSettings = (await settingsApi.get()).agents;
        out.reviewOff = await approve(w.id);
        st().launchAgent("claude-code");
        await sleep(2500);
        out.offLabel = label();
        out.offBuffer = (dumpDevTerminals()[agentPanes()[0]] ?? []).join("\n").slice(0, 200);
        await sessionApi.close(sessionRegistry.get(agentPanes()[0])!);
        await sleep(1000);

        await st().updateSettings((x) => ({ ...x, agents: { ...x.agents, statusSignals: ["claude-code"] } }));
        out.onSettings = (await settingsApi.get()).agents;
        const again = await agentApi.authorize(w.id, "claude-code");
        out.askedAgainAfterTurningOn = again.kind === "required";
        out.reviewOn = await approve(w.id);
        const evs: string[] = [];
        await listen<{ type: string; kind?: string }>("session://lifecycle", (e) => evs.push(`${e.payload.type}:${e.payload.kind ?? ""}`));
        st().launchAgent("claude-code");
        const seen: string[] = [];
        const end = Date.now() + 25000;
        while (Date.now() < end) {
          const l = label();
          if (l && seen[seen.length - 1] !== l) seen.push(l);
          if (l === "Turn finished") break;
          await sleep(150);
        }
        out.labelsSeen = seen;
        out.eventsSeen = evs;
        out.storeStatus = st().sessionStatus;
        const pane = agentPanes()[agentPanes().length - 1];
        out.detail = last("[data-agent-detail]")?.getAttribute("title") ?? null;
        out.buffer = (dumpDevTerminals()[pane] ?? []).join("\n").slice(0, 200);
        // A reload must keep the status (Core owns it), and closing must clear it.
        const sid = sessionRegistry.get(pane)!;
        out.listedStatus = (await sessionApi.list()).find((x) => x.id === sid)?.status ?? null;
        await sessionApi.close(sid);
        await sleep(1500);
        out.afterClose = (await sessionApi.list()).find((x) => x.id === sid)?.status ?? "closed";
        await report({ stage: "status", out });
        return;
      }
      if (visual === "limit") {
        // The agent limit (ADR 0027): one slot. Agent B is approved ONCE; while A holds the slot B must be
        // refused WITHOUT spending that approval, and must start afterwards using the same approval.
        await st().updateSettings((x) => ({ ...x, agents: { ...x.agents, maxConcurrent: 1 } }));
        const mk = async (name: string, args: string[]) => (await agentApi.saveCustom({ name, executable: `${cfg.workspace}/mock-agent`, args, env: [] })).find((a) => a.name === name)!;
        const A = await mk("Agent A", ["--a"]);
        const B = await mk("Agent B", ["--b"]);
        await st().refreshAgents();
        const approve = async (id: string, scope: "once" | "workspace") => {
          const r = await agentApi.authorize(w.id, id);
          if (r.kind === "required") await approvalApi.decide(r.requests[0].id, { decision: "allow", scope });
        };
        const agentPanes = () => Object.entries(st().paneKinds).filter(([, k]) => k.type === "agent").map(([id]) => id);
        const grants = async () => (await approvalApi.list()).active.map((g) => `${g.scope}:${g.summary.split(" at ")[0]}`);
        await approve(A.id, "workspace");
        await approve(B.id, "once");
        st().launchAgent(A.id);
        await sleep(2500);
        const [pa] = agentPanes();
        st().launchAgent(B.id);
        await sleep(2500);
        const pb = agentPanes()[1];
        const refused = { ...st().paneState[pb] };
        const grantsAfterRefusal = await grants(); // B's once grant must still be there
        st().newTab(); // a plain terminal is never limited
        await sleep(2000);
        const sp = activeSpace(st());
        const terminal = { ...st().paneState[sp!.tabs.find((t) => t.id === sp!.activeTabId)!.activePaneId] };
        await sessionApi.close(sessionRegistry.get(pa)!); // free the slot
        await sleep(1500);
        st().restartPane(pb); // Launch again, as the user would
        await sleep(2500);
        const started = { ...st().paneState[pb] };
        const history = (await approvalApi.list()).history.map((g) => `${g.state}:${g.scope}:${g.summary.split(" at ")[0]}`);
        await report({ stage: "limit", refused, grantsAfterRefusal, terminal, started, history, grantsNow: await grants() });
        return;
      }
      if (visual === "reload") {
        // Sessions outlive a page reload (ADR 0026). Phase 1 starts a shell, prints a marker and reloads the
        // page. Phase 2 (the reloaded page) must find the SAME session, see the marker replayed, and still be able to use it.
        const waitFor = async (cond: () => boolean, ms = 10000) => {
          const end = Date.now() + ms;
          while (Date.now() < end && !cond()) await sleep(100);
          return cond();
        };
        const ws = w.id;
        const pane = () => {
          const sp = st().spaces[ws];
          return sp?.tabs.find((t) => t.id === sp.activeTabId)?.activePaneId;
        };
        const text = () => (pane() ? (dumpDevTerminals()[pane()!] ?? []).join("\n") : "");
        if ((st().spaces[ws]?.tabs.length ?? 0) === 0) {
          st().newTab();
          await waitFor(() => st().paneState[pane() ?? ""]?.kind === "running");
          const sid = sessionRegistry.get(pane()!);
          sessionStorage.setItem("smokeSid", String(sid));
          await type(pane()!, "echo MARK_$((40+2))\r");
          const sawMarker = await waitFor(() => text().includes("MARK_42"));
          await sleep(500); // let the prompt settle so the snapshot is what the user saw
          sessionStorage.setItem("smokeLines", JSON.stringify(text().split("\n")));
          await report({ stage: "before-reload", sid, sawMarker });
          await sleep(300);
          location.reload();
          return;
        }
        const pa = pane()!;
        const replayed = await waitFor(() => text().includes("MARK_42"));
        await sleep(500); // replay arrives in chunks; let it finish before comparing
        const replayedLines = text().split("\n");
        const before = JSON.parse(sessionStorage.getItem("smokeLines") ?? "[]") as string[];
        const sidAfter = sessionRegistry.get(pa);
        // A marker unique to this reload, so "the shell still answers" cannot be satisfied by replayed text.
        const reloads = Number(sessionStorage.getItem("smokeN") ?? "0") + 1;
        sessionStorage.setItem("smokeN", String(reloads));
        await type(pa, `echo ALIVE_$((50+${reloads}))\r`);
        const alive = await waitFor(() => text().includes(`ALIVE_${50 + reloads}`));
        sessionStorage.setItem("smokeLines", JSON.stringify(text().split("\n")));
        const results = JSON.parse(sessionStorage.getItem("smokeResults") ?? "[]") as unknown[];
        results.push({ reload: reloads, sameSession: String(sidAfter) === sessionStorage.getItem("smokeSid"), replayMatches: JSON.stringify(replayedLines) === JSON.stringify(before), alive });
        sessionStorage.setItem("smokeResults", JSON.stringify(results));
        await report({ stage: "after-reload", results, reloads, sidBefore: sessionStorage.getItem("smokeSid"), sidAfter, tabs: st().spaces[ws].tabs.length, replayed, replayMatchesWhatWasOnScreen: JSON.stringify(replayedLines) === JSON.stringify(before), before, replayedLines, alive, state: st().paneState[pa]?.kind, lines: text().split("\n") });
        if (reloads < 2) {
          await sleep(300);
          location.reload(); // a second reload must work just as well
        }
        return;
      }
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

    st().launchAgent(mock.id); // no approval yet: the backend must refuse and start nothing
    await sleep(2500);
    const unreviewed = { ...st().paneState[agentPanes()[0]] };

    // Approve once through the real gate (the backend builds the request), then launch.
    const auth = await agentApi.authorize(st().activeWorkspaceId!, mock.id);
    if (auth.kind === "required") await approvalApi.decide(auth.requests[0].id, { decision: "allow", scope: "once" });
    st().launchAgent(mock.id);
    await sleep(2500);
    const running = agentPanes()[1];
    await type(running, "hello\r");
    await sleep(1000);
    const runningState = { ...st().paneState[running] };
    await type(running, "bye\r"); // mock exits 0: an agent pane must NOT auto-close
    await sleep(2000);
    const afterClean = { state: { ...st().paneState[running] }, stillThere: agentPanes().includes(running) };

    st().launchAgent("codex"); // not installed on this machine
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
