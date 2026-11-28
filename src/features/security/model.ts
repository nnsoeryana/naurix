import type { Finding, Severity } from "./api";

/** Most severe first. */
export const SEVERITIES: readonly Severity[] = ["critical", "high", "medium", "low", "info"];

export const SEVERITY_LABEL: Record<Severity, string> = { critical: "Critical", high: "High", medium: "Medium", low: "Low", info: "Info" };

export function countBySeverity(findings: readonly Finding[]): Record<Severity, number> {
  const c: Record<Severity, number> = { critical: 0, high: 0, medium: 0, low: 0, info: 0 };
  for (const f of findings) c[f.severity] += 1;
  return c;
}

/** `src/a.rs:12`, `src/a.rs`, or a dash when the finding has no location. */
export function locationLabel(f: Finding): string {
  if (!f.location) return "—";
  return f.location.line == null ? f.location.path : `${f.location.path}:${f.location.line}`;
}

/** Findings with one of `severities` whose rule, location, message or tool contains `text` (any case). */
export function filterFindings(findings: readonly Finding[], severities: ReadonlySet<Severity>, text: string): Finding[] {
  const q = text.trim().toLowerCase();
  return findings.filter(
    (f) => severities.has(f.severity) && (!q || [f.ruleId, locationLabel(f), f.message, f.tool].some((s) => s.toLowerCase().includes(q))),
  );
}

/** Severity first, then path, then line; findings without a location go last within their severity. */
export function sortFindings(findings: readonly Finding[]): Finding[] {
  const rank = (s: Severity) => SEVERITIES.indexOf(s);
  return [...findings].sort((a, b) => {
    if (a.severity !== b.severity) return rank(a.severity) - rank(b.severity);
    if (!a.location || !b.location) return a.location ? -1 : b.location ? 1 : 0;
    if (a.location.path !== b.location.path) return a.location.path < b.location.path ? -1 : 1;
    return (a.location.line ?? 0) - (b.location.line ?? 0);
  });
}
