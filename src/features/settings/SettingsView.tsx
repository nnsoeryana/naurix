import { ApprovalsPanel } from "../approvals/ApprovalsPanel";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useStore } from "../../state/store";
import { agentApi, type AgentInfo } from "../agents/api";
import { AppearanceSettings } from "../appearance/AppearanceSettings";
import { AGENTS_MAX, AGENTS_MIN, FONT_MAX, FONT_MIN, SCROLLBACK_MAX, SCROLLBACK_MIN, SCROLLBACK_PRESETS, settingsApi, type PerfSnapshot, type SecurityInfo, type Settings } from "./api";

const SECTIONS = ["General", "Appearance", "Terminal", "AI Agents", "MCP Integrations", "Agent Skills", "Performance", "Security"] as const;
type Section = (typeof SECTIONS)[number];

const field = "rounded border border-line bg-midnight px-2 py-1 text-sm outline-none focus:border-purple";
const outline = "rounded-lg border border-line px-3 py-1 text-xs text-cloud hover:bg-raised disabled:opacity-40";

function Row({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[14rem_1fr] items-start gap-4 border-b border-line/60 py-3">
      <div>
        <div className="text-sm">{label}</div>
        {hint && <div className="mt-0.5 text-[11px] leading-snug text-faint">{hint}</div>}
      </div>
      <div className="min-w-0 text-sm">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="flex items-center gap-2">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} aria-label={label} />
      <span className="text-xs text-muted">{checked ? "On" : "Off"}</span>
    </label>
  );
}

/** A text/number input that commits on Enter or blur, so half-typed values never reach validation. */
function Draft({ value, onCommit, placeholder, type = "text", className = "", ...rest }: { value: string; onCommit: (v: string) => void; placeholder?: string; type?: string; className?: string; min?: number; max?: number; "aria-label"?: string }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return <input {...rest} type={type} className={`${field} ${className}`} value={v} placeholder={placeholder} onChange={(e) => setV(e.target.value)} onBlur={() => v !== value && onCommit(v)} onKeyDown={(e) => e.key === "Enter" && (e.currentTarget.blur(), v === value || onCommit(v))} />;
}

/** Settings (M10): every common option, validated by the backend, applied immediately. */
export function SettingsView() {
  const [section, setSection] = useState<Section>("General");
  const settings = useStore((s) => s.settings);
  const update = useStore((s) => s.updateSettings);
  const terminal = (patch: Partial<Settings["terminal"]>) => void update((s) => ({ ...s, terminal: { ...s.terminal, ...patch } }));
  const general = (patch: Partial<Settings["general"]>) => void update((s) => ({ ...s, general: { ...s.general, ...patch } }));
  const agentsCfg = (patch: Partial<Settings["agents"]>) => void update((s) => ({ ...s, agents: { ...s.agents, ...patch } }));
  const setView = useStore((s) => s.setView);
  const setNotice = useStore((s) => s.setNotice);
  const fail = useCallback((e: unknown) => setNotice(String(e)), [setNotice]);

  const [sec, setSec] = useState<SecurityInfo | null>(null);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [perf, setPerf] = useState<PerfSnapshot | null>(null);
  const loadSec = useCallback(() => settingsApi.security().then(setSec).catch(fail), [fail]);
  useEffect(() => {
    void loadSec();
    agentApi.list().then(setAgents).catch(fail);
  }, [loadSec, fail]);

  const t = settings.terminal;
  const rendererStatus = useStore((s) => s.rendererStatus);

  return (
    <div className="absolute inset-0 flex bg-midnight">
      <nav className="w-52 shrink-0 border-r border-line p-3" aria-label="Settings sections">
        <h1 className="mb-3 px-2 text-lg font-semibold">Settings</h1>
        {SECTIONS.map((s) => (
          <button key={s} className={`mb-0.5 block w-full rounded-lg px-3 py-1.5 text-left text-sm ${section === s ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setSection(s)}>{s}</button>
        ))}
      </nav>

      <div className="min-w-0 flex-1 overflow-auto px-10 py-8" data-settings-section={section}>
        <h2 className="mb-2 text-xl font-semibold">{section}</h2>

        {section === "General" && (
          <>
            <Row label="Naurix" hint="Everything stays on this Mac."><span className="font-mono text-xs text-muted">{sec ? `v${sec.version} · ${sec.arch}` : "…"}</span></Row>
            <Row label="Close a shell when it exits" hint="A shell that ends normally (code 0) closes its pane. Failures and agent panes stay open so you can read them."><Toggle label="Close pane on clean exit" checked={settings.general.closeOnCleanExit} onChange={(v) => general({ closeOnCleanExit: v })} /></Row>
            <Row label="Ask before closing running work" hint="Confirm before closing a pane or tab where a command or an agent is still running."><Toggle label="Confirm closing running work" checked={settings.general.confirmCloseRunning} onChange={(v) => general({ confirmCloseRunning: v })} /></Row>
            <Row label="Naurix companion" hint="A small aurora companion at the bottom of the sidebar. It reflects your agents' process state (running, failed, exited) and never changes how they run."><Toggle label="Show Naurix" checked={settings.general.naurixEnabled} onChange={(v) => general({ naurixEnabled: v })} /></Row>
            {settings.general.naurixEnabled && (
              <>
                <Row label="Naurix animations" hint="Off keeps a still Naurix. Animation also stops automatically when macOS Reduce Motion is on or the window is hidden."><Toggle label="Animate Naurix" checked={settings.general.naurixAnimations} onChange={(v) => general({ naurixAnimations: v })} /></Row>
                <Row label="Reduced animation" hint="Slower, calmer movement."><Toggle label="Reduce Naurix animation" checked={settings.general.naurixReduced} onChange={(v) => general({ naurixReduced: v })} /></Row>
              </>
            )}
            <Row label="Reset" hint="Restores every setting on this screen to its default. Projects, agents, MCP servers and skills are not affected."><button className={outline} onClick={() => useStore.getState().setDialog({ kind: "confirm", title: "Reset all settings?", message: "Terminal, general and performance options go back to their defaults.", confirmLabel: "Reset", danger: true, onConfirm: () => void useStore.getState().resetSettings() })}>Reset all settings…</button></Row>
          </>
        )}

        {section === "Appearance" && <AppearanceSettings />}

        {section === "Terminal" && (
          <>
            <Row label="Font" hint={<>Your own font comes first, then Nerd Fonts for prompt icons (install one if icons show as empty boxes; see the README). Leave empty for the default. Try <code>Dank Mono</code> or <code>JetBrains Mono</code>.</>}><Draft aria-label="Font family" className="w-72" value={t.fontFamily ?? ""} placeholder="Default" onCommit={(v) => terminal({ fontFamily: v.trim() || null })} /></Row>
            <Row label="Font size" hint={`${FONT_MIN}–${FONT_MAX}`}><Draft aria-label="Font size" type="number" min={FONT_MIN} max={FONT_MAX} className="w-24" value={String(t.fontSize)} onCommit={(v) => terminal({ fontSize: Number(v) })} /></Row>
            <Row label="Scrollback (lines)" hint={`${SCROLLBACK_MIN}–${SCROLLBACK_MAX}. More lines use more memory per terminal.`}>
              <div className="flex items-center gap-2">
                <Draft aria-label="Scrollback lines" type="number" min={SCROLLBACK_MIN} max={SCROLLBACK_MAX} className="w-28" value={String(t.scrollback)} onCommit={(v) => terminal({ scrollback: Number(v) })} />
                {SCROLLBACK_PRESETS.map((n) => <button key={n} className={`${outline} ${t.scrollback === n ? "border-purple" : ""}`} onClick={() => terminal({ scrollback: n })}>{n.toLocaleString()}</button>)}
              </div>
            </Row>
            <Row label="Cursor"><div className="flex items-center gap-4">
              <select aria-label="Cursor style" className={field} value={t.cursorStyle} onChange={(e) => terminal({ cursorStyle: e.target.value as Settings["terminal"]["cursorStyle"] })}><option value="block">Block</option><option value="bar">Bar</option><option value="underline">Underline</option></select>
              <label className="flex items-center gap-2 text-xs text-muted"><input type="checkbox" checked={t.cursorBlink} onChange={(e) => terminal({ cursorBlink: e.target.checked })} /> Blink</label>
            </div></Row>
            <Row label="Login shell" hint="Starts the shell with -l so your profile loads (PATH, nvm…). A heavy profile makes the first prompt slower; turn it off for a faster start. Applies to new terminals."><Toggle label="Start shells as login shells" checked={t.loginShell} onChange={(v) => terminal({ loginShell: v })} /></Row>
            <Row label="Shell" hint="Leave empty to use your $SHELL. An absolute path to an executable. Applies to new terminals.">
              <div className="flex gap-2">
                <Draft aria-label="Shell path" className="w-72 font-mono" value={t.shellPath ?? ""} placeholder="Use $SHELL" onCommit={(v) => terminal({ shellPath: v.trim() || null })} />
                <button className={outline} onClick={() => void agentApi.pickExecutable().then((p) => { if (p) terminal({ shellPath: p }); })}>Browse…</button>
                {t.shellPath && <button className={outline} onClick={() => terminal({ shellPath: null })}>Use $SHELL</button>}
              </div>
            </Row>
          </>
        )}

        {section === "AI Agents" && (
          <>
            <Row label="Agents running at once" hint={`${AGENTS_MIN}–${AGENTS_MAX}. Starting another while this many run fails and starts nothing. Plain terminals are never limited. Lowering it does not stop agents that are already running.`}>
              <Draft aria-label="Agents running at once" type="number" min={AGENTS_MIN} max={AGENTS_MAX} className="w-24" value={String(settings.agents.maxConcurrent)} onCommit={(v) => agentsCfg({ maxConcurrent: Number(v) })} />
            </Row>
            <p className="py-2 text-sm text-muted">Where each agent&apos;s program is found. Naurix never installs agents. Changing the executable asks you to review the command again.</p>
            {agents.map((a) => (
              <Row key={a.id} label={a.name} hint={a.kind === "custom" ? "Custom agent" : undefined}>
                <div className="space-y-1">
                  <div className={`font-mono text-xs ${a.resolvedPath ? "text-teal" : "text-warning"}`}>{a.resolvedPath ? `✓ ${a.resolvedPath}` : `Not found: ${a.executable}`}</div>
                  <div className="flex gap-2">
                    <Draft aria-label={`${a.name} executable`} className="w-72 font-mono" value={a.pathOverride ?? ""} placeholder={`Use “${a.executable}” from PATH`} onCommit={(v) => void agentApi.setPath(a.id, v.trim() || null).then(setAgents).catch(fail)} />
                    <button className={outline} onClick={() => void agentApi.pickExecutable().then((p) => { if (p) void agentApi.setPath(a.id, p).then(setAgents).catch(fail); })}>Browse…</button>
                  </div>
                </div>
              </Row>
            ))}
            <div className="pt-4"><button className={outline} onClick={() => useStore.getState().openLauncher(true)}>Add or edit custom agents…</button></div>
          </>
        )}

        {section === "MCP Integrations" && (
          <>
            <Row label="Servers" hint="Defined once, approved one by one, then written into the agent configs Naurix has verified."><span className="text-xs text-muted">{sec ? `${sec.mcpServers} defined · ${sec.mcpApproved} approved` : "…"}</span></Row>
            <Row label="Secrets" hint="Tokens and passwords are stored in the system credential store, never in Naurix's database or in agent config files."><span className="font-mono text-xs text-muted">{sec?.secretStore ?? "…"}</span></Row>
            <div className="pt-4"><button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => setView("mcp")}>Open MCP integrations</button></div>
          </>
        )}

        {section === "Agent Skills" && (
          <>
            <Row label="Installed skills" hint="One stored copy each, linked into the agents you choose. Skills are untrusted: Naurix never runs anything inside them."><span className="text-xs text-muted">{sec ? `${sec.skills} installed` : "…"}</span></Row>
            <div className="pt-4"><button className="btn-primary rounded-lg px-3 py-1 text-xs" onClick={() => setView("skills")}>Open agent skills</button></div>
          </>
        )}

        {section === "Performance" && (
          <>
            <Row label="Terminal renderer" hint={<>The default (DOM) is the lowest-risk choice. WebGL draws on the GPU and can be faster under heavy output, but uses more GPU memory; it is only active for terminals on screen. See <code>docs/PERFORMANCE_AND_COMPATIBILITY.md</code> for measurements.</>}>
              <select aria-label="Terminal renderer" className={field} value={t.renderer} onChange={(e) => terminal({ renderer: e.target.value as Settings["terminal"]["renderer"] })}><option value="dom">DOM (default)</option><option value="webgl">WebGL (experimental)</option></select>
              <div className="mt-1 text-[11px] text-faint" data-renderer-status>{rendererStatus === "webgl-active" ? "WebGL is active in the terminal on screen." : rendererStatus === "webgl-unavailable" ? "WebGL could not start here, so the DOM renderer is being used." : "Using the DOM renderer."}</div>
            </Row>
            <Row label="Scrollback" hint="Each terminal keeps this many lines in memory. Lower it if you run many terminals."><span className="text-xs text-muted">{t.scrollback.toLocaleString()} lines (change under Terminal)</span></Row>
            <Row label="Cursor blink" hint="A blinking cursor wakes the renderer repeatedly. Turning it off lowers idle CPU on a focused window."><Toggle label="Blink the cursor" checked={t.cursorBlink} onChange={(v) => terminal({ cursorBlink: v })} /></Row>
            <Row label="Memory now" hint="A single reading when you ask. Naurix does no background sampling. WebKit's helper processes are separate and not included here; scripts/bench in the repository measures the whole app.">
              <div className="flex items-center gap-3">
                <button className={outline} onClick={() => void settingsApi.perf().then(setPerf).catch(fail)}>Measure</button>
                {perf && <span className="font-mono text-xs text-muted" data-perf>{perf.rssMb !== null ? `${perf.rssMb} MB` : "n/a"} in the Naurix process · {perf.sessions} running session{perf.sessions === 1 ? "" : "s"}</span>}
              </div>
            </Row>
          </>
        )}

        {section === "Security" && sec && (
          <>
            <Row label="Telemetry" hint="There is none. Naurix does not collect usage data, and has no analytics or crash reporting."><span className="text-teal text-xs">Off, and not built in</span></Row>
            <Row label="Network" hint="Naurix itself only uses the network when you ask: downloading a skill from Git, or a server you approved. Nothing runs in the background."><span className="text-xs text-muted">On request only</span></Row>
            <Row label="Data folder" hint="Workspaces, session metadata, settings, MCP definitions and skills. Terminal contents and secrets are never stored here."><span className="break-all font-mono text-xs text-muted" data-data-dir>{sec.dataDir}</span></Row>
            <Row label="Credential store" hint={`Secret values for MCP servers (service ${sec.keychainService}).`}><span className="font-mono text-xs text-muted">{sec.secretStore}</span></Row>
            <Row label="MCP approvals" hint="Withdrawing approvals makes every server inert again until you review it. Nothing is deleted.">
              <div className="flex items-center gap-3"><span className="text-xs text-muted">{sec.mcpApproved} of {sec.mcpServers} approved</span><button className={outline} disabled={sec.mcpApproved === 0} onClick={() => useStore.getState().setDialog({ kind: "confirm", title: "Withdraw all MCP approvals?", message: "Servers stay defined, but none may run or be written to an agent until you approve it again.", confirmLabel: "Withdraw", danger: true, onConfirm: () => void settingsApi.revokeMcpApprovals().then(loadSec).catch(fail) })}>Withdraw all…</button></div>
            </Row>
            <Row label="Agent launch approvals" hint="What you allowed agents to start. An approval only matches the exact program and arguments you reviewed; change either and you are asked again. Revoke to be asked next time."><ApprovalsPanel /></Row>
            <Row label="Trusted Git repositories" hint="Repositories whose own configuration defines commands (filters) that Git would run. Revoke to stop running Git there.">
              {sec.trustedRepos.length === 0 ? <span className="text-xs text-faint">None</span> : (
                <ul className="space-y-1">{sec.trustedRepos.map((r) => <li key={r.key} className="flex items-center gap-3"><span className="min-w-0 flex-1 truncate font-mono text-xs text-muted" title={r.path}>{r.path}</span><button className={outline} onClick={() => void settingsApi.untrustRepo(r.key).then(loadSec).then(() => useStore.getState().refreshGit()).catch(fail)}>Revoke</button></li>)}</ul>
              )}
            </Row>
          </>
        )}
      </div>
    </div>
  );
}
