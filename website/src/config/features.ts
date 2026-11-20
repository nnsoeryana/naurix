import type { LucideIcon } from "lucide-react";
import { Bot, FolderKanban, Plug, ShieldCheck, SquareTerminal, Sparkles } from "lucide-react";

/**
 * Status is audited against README.md "What works today" (M10 of 11).
 * Update here when the product changes; the UI reads only this file.
 */
export type FeatureStatus = "available" | "in-development" | "planned";

export interface Feature {
  title: string;
  description: string;
  icon: LucideIcon;
  status: FeatureStatus;
  /** Honest scope note shown under the description. */
  note?: string;
}

export const features: Feature[] = [
  {
    title: "Smart Terminal",
    description: "Multiple terminal tabs and split panes in a modern desktop workspace.",
    icon: SquareTerminal,
    status: "available",
    note: "Real login shell on a PTY, tabs, splits, command palette.",
  },
  {
    title: "Agent Management",
    description: "Launch and manage different AI coding agents.",
    icon: Bot,
    status: "available",
    note: "Launches CLIs and shows process status. It does not read task-level agent state.",
  },
  {
    title: "MCP Integrations",
    description: "Manage Model Context Protocol integrations.",
    icon: Plug,
    status: "available",
    note: "Approve, test, preview and undo sync. Sync targets Claude Code and OpenCode today.",
  },
  {
    title: "Skills Management",
    description: "Organize and manage reusable agent skills.",
    icon: Sparkles,
    status: "available",
    note: "Reviewed install from Git or a folder. Skill scripts are never executed.",
  },
  {
    title: "Workspace Management",
    description: "Keep projects and development sessions organized.",
    icon: FolderKanban,
    status: "available",
    note: "Per-project workspaces plus read-only Git status.",
  },
  {
    title: "Security & Privacy",
    description: "Local-first architecture with an emphasis on user control.",
    icon: ShieldCheck,
    status: "available",
    note: "Secrets in the OS credential store. No terminal content persisted by default.",
  },
];

export const statusLabel: Record<FeatureStatus, string> = {
  available: "Available",
  "in-development": "In development",
  planned: "Planned",
};

/** Agents Naurix can launch today (README). No partnership or endorsement is implied. */
export const agents = [
  { name: "Claude Code", detail: "MCP sync" },
  { name: "OpenAI Codex CLI", detail: "Launch" },
  { name: "Gemini CLI", detail: "Launch" },
  { name: "Antigravity CLI", detail: "Launch" },
  { name: "OpenCode", detail: "MCP sync" },
  { name: "Custom CLI", detail: "Any executable" },
] as const;
