#!/usr/bin/env node
// Measures Naurix memory (ADR 0006): main process + its WebKit helpers.
// Usage: node bench/measure.mjs <app-binary> [--label idle] [--seconds 30] [--interval 2] [--settle 5]
//
// WebKit helper processes are launched via launchd (ppid 1), so they cannot be found by
// walking the process tree. We snapshot helper PIDs before launch and attribute the new ones to
// the app. LIMITATION: opening other WebKit-based apps during a run contaminates the result.
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync, unlinkSync } from "node:fs";
import os from "node:os";

const args = process.argv.slice(2);
const bin = args[0];
if (!bin) { console.error("usage: measure.mjs <app-binary> [--label x] [--seconds n] [--interval n] [--settle n]"); process.exit(2); }
const opt = (n, d) => { const i = args.indexOf(`--${n}`); return i > 0 ? args[i + 1] : d; };
const focus = args.includes("--focus");
const label = opt("label", "idle"), seconds = +opt("seconds", 30), interval = +opt("interval", 2), settle = +opt("settle", 5);

const ps = () => execFileSync("ps", ["-axo", "pid=,ppid=,rss=,pcpu=,comm="]).toString().trim().split("\n").map((l) => {
  const m = l.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+)\s+(.*)$/);
  return m && { pid: +m[1], ppid: +m[2], rssKB: +m[3], cpu: +m[4], comm: m[5] };
}).filter(Boolean);
const isWebKit = (p) => /com\.apple\.WebKit\.(WebContent|GPU|Networking)/.test(p.comm);
const sleep = (s) => new Promise((r) => setTimeout(r, s * 1000));
const footprintKB = (pid) => {
  const f = `${os.tmpdir()}/at-fp-${pid}.json`;
  try {
    execFileSync("footprint", ["-p", String(pid), "--json", f], { stdio: "ignore" });
    const b = JSON.parse(readFileSync(f, "utf8")).processes?.[0]?.footprint;
    return b ? Math.round(b / 1024) : null;
  } catch { return null; } finally { try { unlinkSync(f); } catch {} }
};
const before = new Set(ps().filter(isWebKit).map((p) => p.pid));
const child = spawn(bin, [], { stdio: "ignore" });
console.error(`launched pid ${child.pid}; settling ${settle}s`);
await sleep(settle);

if (focus) {
  // Bring the app window to the front so a focused window (blinking cursor etc.) is measured.
  try { execFileSync("osascript", ["-e", `tell application "System Events" to set frontmost of (first process whose unix id is ${child.pid}) to true`], { stdio: "ignore" }); } catch { console.error("could not focus the window"); }
  await sleep(2);
}
const samples = [];
const end = Date.now() + seconds * 1000;
while (Date.now() < end) {
  const all = ps();
  // The app's own process + its WebKit helpers. Child shells/agents are deliberately excluded.
  const procs = all.filter((p) => p.pid === child.pid || (isWebKit(p) && !before.has(p.pid)));
  const byRole = (f) => procs.filter(f);
  const sum = (a, k) => a.reduce((s, p) => s + p[k], 0);
  samples.push({
    t: new Date().toISOString(),
    processes: procs.length,
    mainRssMB: +(sum(byRole((p) => p.pid === child.pid), "rssKB") / 1024).toFixed(1),
    helpersRssMB: +(sum(byRole((p) => p.pid !== child.pid), "rssKB") / 1024).toFixed(1),
    totalRssMB: +(sum(procs, "rssKB") / 1024).toFixed(1),
    totalFootprintMB: +(procs.map((p) => footprintKB(p.pid) ?? p.rssKB).reduce((a, b) => a + b, 0) / 1024).toFixed(1),
    cpuPct: +sum(procs, "cpu").toFixed(1),
  });
  await sleep(interval);
}
child.kill("SIGTERM");

const med = (k) => { const v = samples.map((s) => s[k]).sort((a, b) => a - b); return v[Math.floor(v.length / 2)]; };
const result = {
  label, arch: os.arch(), cpu: os.cpus()[0].model, macos: execFileSync("sw_vers", ["-productVersion"]).toString().trim(),
  binary: bin, seconds, interval,
  median: { totalRssMB: med("totalRssMB"), totalFootprintMB: med("totalFootprintMB"), cpuPct: med("cpuPct") },
  samples,
};
mkdirSync("bench/results", { recursive: true });
const file = `bench/results/${new Date().toISOString().slice(0, 10)}-${os.arch()}-${label}.json`;
writeFileSync(file, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result.median), "->", file);
