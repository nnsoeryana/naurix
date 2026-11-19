import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

// The core crate serializes these with snake_case keys.
export type Flag = { kind: string; detail: string };
export type Compat = { agent_id: string; status: "ok" | "needs_fix" | "unknown" | "unsupported"; note: string };

export type Candidate = {
  rel_path: string;
  dir_name: string;
  name: string | null;
  description: string | null;
  file_count: number;
  total_bytes: number;
  scripts: string[];
  flags: Flag[];
  compat: Compat[];
  tree_hash: string;
};

export type Inspection = { staging_id: string; source_kind: "git" | "local" | "builtin"; source: string; revision: string; install_root: string; candidates: Candidate[] };

export type Skill = {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  source_kind: "git" | "local" | "builtin";
  source_url: string;
  source_subpath: string;
  revision: string;
  tree_hash: string;
  rev_dir: string;
  prev_rev_dir: string | null;
  flags: Flag[];
  installed_at: string;
  updated_at: string;
};

export type LinkView = { skill_id: string; agent_id: string; scope: "user" | "project"; link_path: string; created_dir: boolean; health: "ok" | "missing" | "replaced" };
export type Target = { agent_id: string; scope: "user" | "project"; dir: string; state: "free" | "linked" | "occupied" };
export type SkillView = { skill: Skill; links: LinkView[]; targets: Target[]; compat: Compat[] };
export type UpdatePlan = { skill_id: string; up_to_date: boolean; staging_id: string; new_revision: string; new_tree_hash: string; added: string[]; removed: string[]; changed: string[]; new_flags: Flag[] };
export type SkillAgent = { agent_id: string; name: string; supported: boolean; user_dir: string | null; project_dir: string | null; note: string };
export type UninstallReport = { removed_links: string[]; left_alone: string[] };

export const skillApi = {
  list: (workspaceId: string | null) => invoke<SkillView[]>("skill_list", { workspaceId }),
  agents: () => invoke<SkillAgent[]>("skill_agents"),
  inspectGit: (url: string) => invoke<Inspection>("skill_inspect_git", { url }),
  inspectLocal: (path: string) => invoke<Inspection>("skill_inspect_local", { path }),
  install: (i: Inspection, selections: { rel_path: string; tree_hash: string }[]) =>
    invoke<Skill[]>("skill_install", { sourceKind: i.source_kind, source: i.source, stagingId: i.staging_id, revision: i.revision, selections }),
  discardStaging: (stagingId: string) => invoke<void>("skill_discard_staging", { stagingId }),
  setLink: (skillId: string, agentId: string, scope: string, enabled: boolean, workspaceId: string | null) => invoke<SkillView[]>("skill_set_link", { skillId, agentId, scope, enabled, workspaceId }),
  checkUpdate: (skillId: string) => invoke<UpdatePlan>("skill_check_update", { skillId }),
  applyUpdate: (p: UpdatePlan) => invoke<Skill>("skill_apply_update", { skillId: p.skill_id, stagingId: p.staging_id, expectedHash: p.new_tree_hash, newRevision: p.new_revision }),
  revert: (skillId: string) => invoke<Skill>("skill_revert", { skillId }),
  uninstall: (skillId: string) => invoke<UninstallReport>("skill_uninstall", { skillId }),
  pickFolder: async (): Promise<string | null> => {
    const r = await open({ directory: true, multiple: false, title: "Choose a skill folder" });
    return typeof r === "string" ? r : null;
  },
};

export const agentLabel: Record<string, string> = { "claude-code": "Claude Code", codex: "Codex CLI", "antigravity-cli": "Antigravity CLI", opencode: "OpenCode", "gemini-cli": "Gemini CLI" };

export function sourceLine(s: Pick<Skill, "source_kind" | "source_url" | "source_subpath" | "revision">): string {
  if (s.source_kind === "builtin") return "Built in to Naurix";
  const where = s.source_subpath ? `${s.source_url} · ${s.source_subpath}` : s.source_url;
  return s.source_kind === "git" ? `${where} @ ${s.revision.slice(0, 8)}` : where;
}
