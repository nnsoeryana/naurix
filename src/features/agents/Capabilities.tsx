import type { AgentInfo, Capability } from "./api";

const labels: Record<Capability["feature"], string> = {
  pty_launch: "Launch in a terminal",
  mcp_config: "MCP configuration",
  skills: "Skills",
  status_signals: "Task status (waiting / done)",
  session_resume: "Resume sessions",
};

function describe(c: Capability): { mark: string; text: string; tone: string } {
  if (c.level === "verified") return { mark: "✓", text: "Verified", tone: "text-teal" };
  if (c.level === "documented_accepted") return { mark: "◐", text: "Link only, not verified", tone: "text-purple" };
  if (c.level === "documented_only") return { mark: "○", text: "Documented, not enabled", tone: "text-warning" };
  return { mark: "–", text: "Not supported yet", tone: "text-faint" };
}

/** Honest capability list for one agent (AG-011). Only "Verified" items are ever acted on. */
export function Capabilities({ agent }: { agent: AgentInfo }) {
  const a = agent.adapter;
  if (!a) {
    return <p className="mb-3 text-xs text-faint">Custom agents use the generic launch only. Nothing else about them is assumed.</p>;
  }
  return (
    <details className="mb-3 text-xs">
      <summary className="cursor-pointer text-muted hover:text-cloud">What Naurix can do with this agent{a.verifiedWith ? ` (verified with ${a.verifiedWith})` : ""}</summary>
      <ul className="mt-2 space-y-1">
        {a.capabilities.map((c) => {
          const d = describe(c);
          const why = c.level === "verified" ? c.evidence : c.level === "documented_accepted" ? `${c.basis}\nSource: ${c.source}` : c.level === "documented_only" ? `Source: ${c.source}` : c.reason;
          return (
            <li key={c.feature} className="flex items-baseline gap-2" title={[c.detail, why].filter(Boolean).join("\n")}>
              <span className={`w-3 shrink-0 text-center ${d.tone}`}>{d.mark}</span>
              <span className="text-cloud/80">{labels[c.feature]}</span>
              <span className={`ml-auto ${d.tone}`}>{d.text}</span>
            </li>
          );
        })}
      </ul>
    </details>
  );
}
