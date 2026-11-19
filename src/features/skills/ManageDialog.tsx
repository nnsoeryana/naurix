import { Modal } from "../../components/Modal";
import { agentLabel, type SkillAgent, type SkillView } from "./api";

const ghost = "rounded px-3 py-1 text-sm bg-raised hover:bg-line";

/** Per-agent switches (user folder / this project's folder). Enabling = a symlink; disabling = removing it. */
export function ManageDialog({ view, agents, workspaceName, onToggle, onClose }: { view: SkillView; agents: SkillAgent[]; workspaceName: string | null; onToggle: (agentId: string, scope: "user" | "project", enabled: boolean) => void; onClose: () => void }) {
  const t = (a: string, s: "user" | "project") => view.targets.find((x) => x.agent_id === a && x.scope === s);
  return (
    <Modal title={`Use “${view.skill.name}” with…`} onClose={onClose} wide>
      <p className="mb-3 text-xs text-muted">Naurix adds a link named <code className="font-mono text-cloud">{view.skill.slug}</code> in the folder each agent's documentation names, only where nothing exists yet, and removes it again when you switch it off. Whether an agent then loads the skill has not been verified.</p>
      <table className="w-full text-sm">
        <thead><tr className="text-left text-[11px] uppercase tracking-wide text-faint"><th className="py-1">Agent</th><th>All projects</th><th>{workspaceName ? `Project: ${workspaceName}` : "This project"}</th></tr></thead>
        <tbody>
          {agents.filter((a) => a.supported).map((a) => (
            <tr key={a.agent_id} className="border-t border-line align-top">
              <td className="py-2 pr-3">{agentLabel[a.agent_id] ?? a.name}</td>
              {(["user", "project"] as const).map((scope) => {
                const target = t(a.agent_id, scope);
                const on = target?.state === "linked";
                const occupied = target?.state === "occupied";
                const disabled = !target || occupied;
                return (
                  <td key={scope} className="py-2 pr-3">
                    <label className={`flex items-center gap-2 ${disabled ? "opacity-60" : ""}`}>
                      <input type="checkbox" checked={on} disabled={disabled} onChange={(e) => onToggle(a.agent_id, scope, e.target.checked)} />
                      <span className="text-xs">{on ? "On" : occupied ? "Name taken" : target ? "Off" : "Choose a project first"}</span>
                    </label>
                    <div className="mt-0.5 break-all font-mono text-[10px] text-faint">{target ? target.dir : scope === "user" ? a.user_dir : a.project_dir}</div>
                    {occupied && <div className="text-[10px] text-warning">Something else is already there, so Naurix left it alone.</div>}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-3 text-[11px] text-faint">Some agents also read other agents' folders (for example OpenCode reads <code>~/.claude/skills</code> and <code>~/.agents/skills</code>), so a skill can show up in more than one place.</p>
      <div className="mt-4 flex justify-end"><button className={ghost} onClick={onClose}>Done</button></div>
    </Modal>
  );
}
