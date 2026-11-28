import { IconBolt, IconFlow, IconFolder, IconNote, IconGear, IconGit, IconHome, IconPencil, IconPlug, IconPlus, IconClose, IconShield } from "../../components/icons";
import { IS_MAC, SHORTCUTS } from "../../shortcuts";
import { useStore } from "../../state/store";
import markUrl from "../../assets/naurix-mark.svg";
import { AgentSessions } from "../agents/AgentSessions";
import { NaurixCompanion } from "../naurix/components/NaurixCompanion";

const row = "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-sm";


export function Sidebar() {
  const workspaces = useStore((s) => s.workspaces);
  const activeId = useStore((s) => s.activeWorkspaceId);
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const select = useStore((s) => s.openWorkspace);
  const add = useStore((s) => s.addWorkspace);
  const remove = useStore((s) => s.removeWorkspaceWithConfirm);
  const rename = useStore((s) => s.renameWorkspaceWithPrompt);

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-sidebar">
      {/* macOS uses an overlay title bar: this strip sits under the traffic lights and drags the window.
          Linux and Windows keep the native title bar, so no spacer is needed. */}
      {IS_MAC ? <div data-tauri-drag-region className="h-10 shrink-0" /> : <div className="h-3 shrink-0" />}
      <div className="flex items-center gap-2.5 px-4 pb-4">
        <img src={markUrl} alt="" className="h-6 w-auto" />
        <span className="text-xl font-semibold tracking-tight">Naurix</span>
      </div>

      <nav className="min-h-0 flex-1 space-y-0.5 overflow-auto px-2" aria-label="Main">
        <button className={`${row} ${view === "home" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("home")}>
          <IconHome className={view === "home" ? "text-purple" : ""} /> Home
        </button>

        <div className="flex items-center">
          <div className={`${row} flex-1 text-muted`}>
            <IconFolder /> Projects
          </div>
          <button className="rounded p-1 text-faint hover:bg-raised hover:text-cloud" onClick={() => void add()} title={`Open project folder (${SHORTCUTS.addWorkspace.label})`} aria-label="Open project folder">
            <IconPlus />
          </button>
        </div>
        <ul className="mb-1 ml-3 border-l border-line pl-2">
          {workspaces.map((w) => (
            <li key={w.id} className={`group flex items-center rounded-lg px-2 py-1 ${w.id === activeId ? "bg-raised text-cloud" : "text-muted hover:bg-raised/60"}`}>
              <button className="min-w-0 flex-1 text-left" onClick={() => select(w.id)} title={w.rootPath}>
                <div className="flex items-center gap-1 truncate text-[13px]">
                  {!w.available && <span className="text-warning" title="Folder not found">⚠</span>}
                  <span className="truncate">{w.name}</span>
                </div>
              </button>
              <span className="invisible flex gap-0.5 group-hover:visible">
                <button aria-label={`Rename ${w.name}`} className="rounded p-1 text-faint hover:bg-line hover:text-cloud" onClick={() => rename(w.id)}><IconPencil /></button>
                <button aria-label={`Remove ${w.name}`} className="rounded p-1 text-faint hover:bg-line hover:text-cloud" onClick={() => remove(w.id)}><IconClose /></button>
              </span>
            </li>
          ))}
          {workspaces.length === 0 && <li className="px-2 py-1 text-xs text-faint">No projects yet</li>}
        </ul>

        <AgentSessions />

        <button className={`${row} ${view === "mcp" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("mcp")}>
          <IconPlug className={view === "mcp" ? "text-purple" : ""} /> MCP
        </button>

        <button className={`${row} ${view === "skills" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("skills")}>
          <IconBolt className={view === "skills" ? "text-purple" : ""} /> Skills
        </button>

        <button className={`${row} ${view === "workflows" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("workflows")}>
          <IconFlow className={view === "workflows" ? "text-purple" : ""} /> Workflows
        </button>

        <button className={`${row} ${view === "memory" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("memory")}>
          <IconNote className={view === "memory" ? "text-purple" : ""} /> Memory
        </button>

        <button className={`${row} ${view === "security" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("security")}>
          <IconShield className={view === "security" ? "text-purple" : ""} /> Security
        </button>

        <button className={`${row} ${view === "git" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("git")}>
          <IconGit className={view === "git" ? "text-purple" : ""} /> Git
        </button>

        <button className={`${row} ${view === "settings" ? "bg-purple/20 text-cloud" : "text-muted hover:bg-raised/60"}`} onClick={() => setView("settings")}>
          <IconGear className={view === "settings" ? "text-purple" : ""} /> Settings
        </button>
      </nav>

      <NaurixCompanion />
    </aside>
  );
}
