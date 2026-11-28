import { useEffect, useMemo, useState } from "react";
import { useStore } from "../../state/store";
import { agentApi } from "../agents/api";
import { securityApi, type Comparison, type Finding, type Opened, type Severity } from "./api";
import { countBySeverity, filterFindings, locationLabel, SEVERITIES, SEVERITY_LABEL, sortFindings } from "./model";

const outline = "rounded border border-line px-2 py-0.5 text-xs text-muted hover:bg-raised disabled:opacity-40";
const input = "w-full rounded border border-line bg-transparent px-2 py-1 text-sm";
/** More rows than this are counted, not drawn; filtering narrows the list. */
const MAX_ROWS = 1000;

// Colour is a second cue only: the severity is always written in normal text (contrast, as in ADR 0030's graph).
const DOT: Record<Severity, string> = { critical: "bg-danger", high: "bg-danger/60", medium: "bg-warning", low: "bg-info", info: "bg-faint" };

type Shown = { kind: "report"; opened: Opened } | { kind: "compare"; baseline: Opened; current: Opened; comparison: Comparison };

/**
 * Security Suite (ADR 0032). Reads a SARIF report the user picks, or compares two. What is shown lives in
 * this component only: leaving the view or switching project forgets it. Red, Blue and Purple are roles on
 * custom agents and skills; this view never starts anything.
 */
export function SecurityView() {
  const wsId = useStore((s) => s.activeWorkspaceId);
  const workspace = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId));
  const agents = useStore((s) => s.agents);
  const setAgents = useStore((s) => s.setAgents);
  const [shown, setShown] = useState<Shown | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Read-only: lists definitions, starts nothing. Keeps the team list current.
  useEffect(() => {
    agentApi.list().then(setAgents).catch(() => {});
  }, [setAgents]);

  // A report belongs to the project it was opened in.
  useEffect(() => {
    setShown(null);
    setError(null);
  }, [wsId]);

  if (!wsId || !workspace) return <div className="p-6 text-sm text-muted">Open a project to read security reports.</div>;

  const act = <T,>(p: Promise<T>, then: (v: T) => void) => {
    setBusy(true);
    p.then((v) => (setError(null), then(v)))
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const open = () => act(securityApi.openReport(wsId), (r) => r.kind === "opened" && setShown({ kind: "report", opened: r }));
  const compare = () =>
    act(securityApi.compare(wsId), (r) => r.kind === "compared" && setShown({ kind: "compare", baseline: r.baseline, current: r.current, comparison: r.comparison }));

  const roles = (["red", "blue", "purple"] as const).map((role) => ({ role, names: agents.filter((a) => a.role === role).map((a) => a.name) }));

  return (
    <div className="h-full overflow-auto p-6" data-security>
      <div className="mx-auto max-w-5xl space-y-4">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold">Security</h1>
          <span className="text-xs text-faint">{workspace.name}</span>
        </div>
        <p className="text-xs text-muted">
          Open a SARIF report that a scanner or an agent wrote, or compare two to see what is new. Naurix reads a report only when you pick it, keeps nothing, and forgets it when you leave this view.
          Reports are untrusted: paths outside this project are not shown, and messages that look like a secret are hidden.
        </p>

        <div className="flex flex-wrap gap-2">
          <button className="btn-primary rounded-lg px-3 py-1 text-xs" disabled={busy} onClick={open} data-security-open>Open report…</button>
          <button className={outline} disabled={busy} onClick={compare} data-security-compare>Compare two reports…</button>
          {shown && <button className={outline} onClick={() => setShown(null)}>Close</button>}
        </div>
        {error && <p className="rounded border border-danger/40 p-2 text-xs text-danger" role="alert" data-security-error>{error}</p>}

        {!shown && (
          <section className="space-y-1 rounded-lg border border-line p-3 text-xs text-muted">
            <p className="text-sm text-cloud">Teams</p>
            {roles.map(({ role, names }) => (
              <p key={role}>
                <span className="capitalize text-cloud">{role} team:</span> {names.length ? names.join(", ") : "no agent yet"}
              </p>
            ))}
            <p className="text-faint">
              Give a custom agent a security role in the agent launcher. Run scanners as custom agents or workflow steps; list exit code 1 as a success when a scanner uses it to report findings.
              Install a team&apos;s instructions as skills.
            </p>
          </section>
        )}

        {shown?.kind === "report" && <ReportPanel opened={shown.opened} />}
        {shown?.kind === "compare" && <ComparePanel baseline={shown.baseline} current={shown.current} comparison={shown.comparison} />}
      </div>
    </div>
  );
}

function Notices({ opened }: { opened: Opened }) {
  const r = opened.report;
  const notes = [
    r.notShown > 0 && `${r.notShown} more results were over the limit and are not shown.`,
    r.pathsDropped > 0 && `${r.pathsDropped} results named a path outside this project; they are shown without a location.`,
    r.messagesHidden > 0 && `${r.messagesHidden} messages looked like they held a secret and are hidden.`,
  ].filter(Boolean);
  return notes.length ? (
    <ul className="space-y-0.5 text-xs text-warning" data-security-notices>
      {notes.map((n) => <li key={n as string}>{n}</li>)}
    </ul>
  ) : null;
}

function ReportPanel({ opened }: { opened: Opened }) {
  const [severities, setSeverities] = useState<Set<Severity>>(() => new Set(SEVERITIES));
  const [text, setText] = useState("");
  const counts = useMemo(() => countBySeverity(opened.report.findings), [opened]);
  const rows = useMemo(() => sortFindings(filterFindings(opened.report.findings, severities, text)), [opened, severities, text]);
  const toggle = (s: Severity) =>
    setSeverities((cur) => {
      const next = new Set(cur);
      if (next.has(s)) next.delete(s);
      else next.add(s);
      return next;
    });

  return (
    <section className="space-y-3" data-security-report>
      <div className="text-sm">
        <span className="text-cloud">{opened.fileName}</span>
        <span className="text-faint"> · {opened.report.tools.join(", ") || "unknown tool"} · {opened.report.findings.length} findings</span>
      </div>
      <Notices opened={opened} />
      <div className="flex flex-wrap items-center gap-2">
        {SEVERITIES.map((s) => (
          <label key={s} className="flex items-center gap-1 rounded border border-line px-2 py-0.5 text-xs">
            <input type="checkbox" checked={severities.has(s)} onChange={() => toggle(s)} />
            <span className={`inline-block h-2 w-2 rounded-full ${DOT[s]}`} aria-hidden />
            {SEVERITY_LABEL[s]} ({counts[s]})
          </label>
        ))}
        <input aria-label="Filter findings" className={`${input} max-w-xs`} placeholder="Filter by rule, file or message" value={text} onChange={(e) => setText(e.target.value)} />
      </div>
      <FindingList findings={rows} empty="No findings match." />
    </section>
  );
}

function ComparePanel({ baseline, current, comparison }: { baseline: Opened; current: Opened; comparison: Comparison }) {
  const groups: { title: string; note: string; items: Finding[]; attr: string }[] = [
    { title: "New", note: `In ${current.fileName} only.`, items: sortFindings(comparison.new), attr: "new" },
    { title: "Still present", note: "In both reports.", items: sortFindings(comparison.stillPresent), attr: "still" },
    {
      title: "No longer reported",
      note: `In ${baseline.fileName} only. The scanner stopped reporting these; that does not prove they were fixed.`,
      items: sortFindings(comparison.noLongerReported),
      attr: "gone",
    },
  ];
  return (
    <section className="space-y-3" data-security-comparison>
      <div className="text-sm">
        <span className="text-cloud">{baseline.fileName}</span>
        <span className="text-faint"> → </span>
        <span className="text-cloud">{current.fileName}</span>
      </div>
      <Notices opened={baseline} />
      <Notices opened={current} />
      {comparison.incomplete && <p className="text-xs text-warning">A report had more results than are read, so these lists may be wrong.</p>}
      {groups.map((g) => (
        <div key={g.title} className="space-y-1" data-security-group={g.attr}>
          <h2 className="text-sm font-semibold">
            {g.title} ({g.items.length})
          </h2>
          <p className="text-xs text-faint">{g.note}</p>
          <FindingList findings={g.items} empty="None." />
        </div>
      ))}
    </section>
  );
}

function FindingList({ findings, empty }: { findings: Finding[]; empty: string }) {
  if (findings.length === 0) return <p className="text-xs text-faint">{empty}</p>;
  const shown = findings.slice(0, MAX_ROWS);
  return (
    <>
      <ul className="divide-y divide-line rounded-lg border border-line">
        {shown.map((f, i) => (
          <li key={`${f.fingerprint}-${i}`} className="space-y-0.5 px-3 py-2" data-finding>
            <div className="flex items-center gap-2 text-xs">
              <span className={`inline-block h-2 w-2 shrink-0 rounded-full ${DOT[f.severity]}`} aria-hidden />
              <span className="w-14 shrink-0 text-cloud">{SEVERITY_LABEL[f.severity]}</span>
              <span className="min-w-0 truncate font-mono text-cloud" title={f.ruleId}>{f.ruleId}</span>
              <span className="ml-auto min-w-0 truncate font-mono text-muted" title={locationLabel(f)}>{locationLabel(f)}</span>
            </div>
            {/* Plain text: report content is never interpreted as markup or links. */}
            <p className="whitespace-pre-wrap break-words text-xs text-muted">{f.message}</p>
            <p className="text-[10px] text-faint">{f.tool}</p>
          </li>
        ))}
      </ul>
      {findings.length > MAX_ROWS && <p className="text-xs text-faint">Showing {MAX_ROWS} of {findings.length}. Filter to narrow the list.</p>}
    </>
  );
}
