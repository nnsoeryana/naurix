import { describe, expect, it } from "vitest";
import type { Finding, Severity } from "./api";
import { countBySeverity, filterFindings, locationLabel, SEVERITIES, sortFindings } from "./model";

const f = (severity: Severity, path: string | null, line: number | null, over: Partial<Finding> = {}): Finding => ({
  tool: "scanner",
  ruleId: "R",
  severity,
  level: "warning",
  location: path === null ? null : { path, line },
  message: "m",
  fingerprint: `${severity}${path}${line}`,
  ...over,
});

describe("security report model", () => {
  it("counts by severity, including zero buckets", () => {
    expect(countBySeverity([f("high", "a", 1), f("high", "b", 1), f("info", null, null)])).toEqual({ critical: 0, high: 2, medium: 0, low: 0, info: 1 });
  });

  it("labels locations", () => {
    expect(locationLabel(f("low", "src/a.rs", 12))).toBe("src/a.rs:12");
    expect(locationLabel(f("low", "src/a.rs", null))).toBe("src/a.rs");
    expect(locationLabel(f("low", null, null))).toBe("—");
  });

  it("filters by severity and by text in rule, location, message or tool", () => {
    const all = [f("critical", "src/auth.rs", 3, { ruleId: "sql-injection" }), f("low", "README.md", 1, { message: "Weak Hash" }), f("medium", null, null, { tool: "Semgrep" })];
    const every = new Set(SEVERITIES);
    expect(filterFindings(all, every, "").length).toBe(3);
    expect(filterFindings(all, every, "SQL").map((x) => x.ruleId)).toEqual(["sql-injection"]);
    expect(filterFindings(all, every, "auth.rs:3").length).toBe(1);
    expect(filterFindings(all, every, "weak hash").length).toBe(1);
    expect(filterFindings(all, every, "semgrep").length).toBe(1);
    expect(filterFindings(all, new Set<Severity>(["low"]), "").map((x) => x.severity)).toEqual(["low"]);
  });

  it("sorts most severe first, then by path and line, located before unlocated", () => {
    const sorted = sortFindings([f("low", "b", 1), f("critical", null, null), f("critical", "b", 9), f("critical", "a", 20), f("critical", "a", 3)]);
    expect(sorted.map(locationLabel)).toEqual(["a:3", "a:20", "b:9", "—", "b:1"]);
  });
});
