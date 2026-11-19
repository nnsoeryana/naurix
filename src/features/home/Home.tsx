import { SHORTCUTS } from "../../shortcuts";
import { tildePath } from "../workspaces/path";
import { useStore } from "../../state/store";

const outline = "rounded-lg border border-line px-4 py-2 text-sm text-cloud hover:bg-raised";

/** Welcome screen (brand mock-up: terminal-style greeting) with the actions that make sense now. */
export function Home() {
  const workspaces = useStore((s) => s.workspaces);
  const activeId = useStore((s) => s.activeWorkspaceId);
  const active = workspaces.find((w) => w.id === activeId);
  const others = workspaces.filter((w) => w.id !== activeId).slice(0, 4);
  const previous = useStore((s) => (activeId ? s.previous[activeId] : undefined)) ?? [];
  const add = useStore((s) => s.addWorkspace);
  const newTab = useStore((s) => s.newTab);
  const select = useStore((s) => s.openWorkspace);
  const launcher = useStore((s) => s.openLauncher);
  const remove = useStore((s) => s.removeWorkspaceWithConfirm);

  return (
    <div className="absolute inset-0 overflow-auto bg-midnight">
      <div className="aurora-glow pointer-events-none absolute inset-x-0 bottom-0 h-3/5" />
      <div className="relative mx-auto max-w-2xl px-10 pb-16 pt-20">
        <p className="font-mono text-xl">
          <span className="text-teal">&gt;</span> Welcome to Naurix <span aria-hidden>👋</span>
        </p>
        <p className="mt-1 font-mono text-sm text-faint">Your terminal. Your agents. Your universe.</p>
        <span className="nx-cursor mt-3 inline-block h-4 w-2 bg-purple/70" aria-hidden />

        <section className="mt-12">
          {!active ? (
            <>
              <h1 className="text-2xl font-semibold">
                <span className="aurora-text">One workspace.</span> Infinite possibilities.
              </h1>
              <p className="mt-2 max-w-md text-sm text-muted">Open a project folder to start. You can run a shell and AI coding agents in it, side by side.</p>
              <button className="btn-primary mt-6 rounded-lg px-4 py-2 text-sm" onClick={() => void add()}>
                Open project folder… <span className="ml-1 opacity-70">{SHORTCUTS.addWorkspace.label}</span>
              </button>
            </>
          ) : !active.available ? (
            <>
              <h1 className="text-2xl font-semibold">Folder not found</h1>
              <p className="mt-2 max-w-md text-sm text-muted">
                <span className="font-mono">{active.rootPath}</span> no longer exists. Move it back, or remove this project from Naurix (your files are never touched).
              </p>
              <button className="mt-6 btn-danger rounded-lg px-4 py-2 text-sm" onClick={() => remove(active.id)}>Remove project…</button>
            </>
          ) : (
            <>
              <h1 className="text-2xl font-semibold">{active.name}</h1>
              <p className="mt-1 font-mono text-xs text-faint">{tildePath(active.rootPath)}</p>
              <div className="mt-6 flex flex-wrap gap-3">
                <button className="btn-primary rounded-lg px-4 py-2 text-sm" onClick={newTab}>New terminal <span className="ml-1 opacity-70">{SHORTCUTS.newTab.label}</span></button>
                <button className={outline} onClick={() => launcher(true)}>Launch AI agent <span className="ml-1 text-faint">{SHORTCUTS.launchAgent.label}</span></button>
                <button className={outline} onClick={() => void add()}>Open another folder…</button>
              </div>
            </>
          )}
        </section>

        {others.length > 0 && (
          <section className="mt-10">
            <h2 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-faint">Other projects</h2>
            <ul className="space-y-1">
              {others.map((w) => (
                <li key={w.id}>
                  <button className="flex w-full items-baseline justify-between gap-4 rounded-lg px-3 py-1.5 text-left hover:bg-raised/60" onClick={() => select(w.id)}>
                    <span className="text-sm">{w.name}</span>
                    <span className="truncate font-mono text-xs text-faint">{tildePath(w.rootPath)}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}

        {active?.available && previous.length > 0 && (
          <section className="mt-10">
            <h2 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-faint">Previous sessions (metadata only)</h2>
            <ul className="space-y-0.5 text-xs text-faint">
              {previous.slice(0, 5).map((r) => (
                <li key={r.id} className="flex justify-between gap-2 px-3">
                  <span className="truncate">{r.title}</span>
                  <span>{r.lifecycle === "interrupted" ? "interrupted when the app closed" : r.lifecycle}</span>
                </li>
              ))}
            </ul>
            <p className="mt-1 px-3 text-xs text-faint">Running processes are not restored after the app quits.</p>
          </section>
        )}

        <p className="mt-16 text-[10px] uppercase tracking-[0.2em] text-faint">Open · Lightweight · Extensible</p>
      </div>
    </div>
  );
}
