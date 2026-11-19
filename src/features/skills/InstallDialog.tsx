import { useState } from "react";
import { Modal } from "../../components/Modal";
import { agentLabel, skillApi, type Candidate, type Inspection } from "./api";

const field = "w-full rounded border border-line bg-midnight px-2 py-1 text-sm outline-none focus:border-purple";
const ghost = "rounded px-3 py-1 text-sm bg-raised hover:bg-line";
const tone = { ok: "text-teal", needs_fix: "text-warning", unknown: "text-faint", unsupported: "text-faint" } as const;
const mark = { ok: "✓", needs_fix: "!", unknown: "?", unsupported: "–" } as const;
const risky = (c: Candidate) => c.flags.some((f) => ["scripts", "shell_injection", "allows_shell_tools", "defines_hooks"].includes(f.kind));

/** Step 1: pick a source and inspect it. Step 2: review every skill (source, risks, compatibility) and choose. */
export function InstallDialog({ onDone, onClose }: { onDone: () => void; onClose: () => void }) {
  const [mode, setMode] = useState<"git" | "local">("git");
  const [url, setUrl] = useState("");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ins, setIns] = useState<Inspection | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());

  const close = () => {
    if (ins?.staging_id) void skillApi.discardStaging(ins.staging_id);
    onClose();
  };

  const inspect = () => {
    setError(null);
    setBusy(mode === "git" ? "Downloading and inspecting… (nothing is run)" : "Inspecting…");
    (mode === "git" ? skillApi.inspectGit(url.trim()) : skillApi.inspectLocal(path.trim()))
      .then((r) => {
        setIns(r);
        setPicked(new Set(r.candidates.length === 1 && !risky(r.candidates[0]) ? [r.candidates[0].rel_path] : []));
      })
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  const install = () => {
    if (!ins) return;
    setBusy("Installing…");
    const sel = ins.candidates.filter((c) => picked.has(c.rel_path)).map((c) => ({ rel_path: c.rel_path, tree_hash: c.tree_hash }));
    skillApi
      .install(ins, sel)
      .then(() => {
        setIns(null);
        onDone();
      })
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(null));
  };

  return (
    <Modal title={ins ? "Review before installing" : "Install skills"} onClose={close} wide>
      {!ins ? (
        <>
          <div className="mb-3 flex gap-2 text-xs">
            <button className={`rounded px-3 py-1 ${mode === "git" ? "bg-purple/20 text-cloud" : "bg-raised text-muted"}`} onClick={() => setMode("git")}>Git repository</button>
            <button className={`rounded px-3 py-1 ${mode === "local" ? "bg-purple/20 text-cloud" : "bg-raised text-muted"}`} onClick={() => setMode("local")}>Folder on this Mac</button>
          </div>
          {mode === "git" ? (
            <label className="block text-xs text-faint">HTTPS URL of a repository containing skills (folders with a SKILL.md)
              <input className={`${field} mt-1 font-mono`} autoFocus value={url} placeholder="https://github.com/owner/skills" onChange={(e) => setUrl(e.target.value)} />
            </label>
          ) : (
            <label className="block text-xs text-faint">A skill folder, or a folder that contains several
              <div className="mt-1 flex gap-2">
                <input className={`${field} font-mono`} value={path} placeholder="/Users/you/skills/my-skill" onChange={(e) => setPath(e.target.value)} />
                <button type="button" className={ghost} onClick={() => void skillApi.pickFolder().then((p) => p && setPath(p))}>Browse…</button>
              </div>
            </label>
          )}
          <p className="mt-3 text-[11px] text-faint">Skills are untrusted content. Naurix downloads and copies files, and never runs anything from a skill. Nothing is installed until you review it on the next screen.</p>
          {busy && <p className="mt-2 text-xs text-muted">{busy}</p>}
          {error && <p className="mt-2 text-xs text-danger">{error}</p>}
          <div className="mt-4 flex justify-end gap-2">
            <button className={ghost} onClick={close}>Cancel</button>
            <button className="btn-primary rounded px-3 py-1 text-sm disabled:opacity-40" disabled={!!busy || !(mode === "git" ? url.trim() : path.trim())} onClick={inspect}>Inspect</button>
          </div>
        </>
      ) : (
        <>
          <dl className="mb-3 space-y-1 text-xs text-muted">
            <div><dt className="inline text-faint">Source: </dt><dd className="inline break-all font-mono">{ins.source}{ins.revision ? ` @ ${ins.revision.slice(0, 8)}` : ""}</dd></div>
            <div><dt className="inline text-faint">Will be stored in: </dt><dd className="inline break-all font-mono">{ins.install_root}</dd></div>
          </dl>
          <ul className="max-h-[50vh] space-y-2 overflow-auto">
            {ins.candidates.map((c) => (
              <li key={c.rel_path} className="rounded-lg border border-line p-3" data-skill-candidate={c.name ?? c.dir_name}>
                <label className="flex items-start gap-3">
                  <input type="checkbox" className="mt-1" checked={picked.has(c.rel_path)} onChange={(e) => setPicked((p) => { const n = new Set(p); if (e.target.checked) n.add(c.rel_path); else n.delete(c.rel_path); return n; })} />
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium">{c.name ?? c.dir_name} <span className="font-mono text-[11px] font-normal text-faint">{c.rel_path || "(folder root)"}</span></div>
                    {c.description && <p className="text-xs text-muted">{c.description}</p>}
                    <p className="mt-1 text-[11px] text-faint">{c.file_count} files · {(c.total_bytes / 1024).toFixed(1)} KB</p>
                    {c.flags.map((f) => (
                      <p key={f.kind} className={`mt-1 rounded border p-1.5 text-[11px] ${["scripts", "shell_injection", "allows_shell_tools", "defines_hooks"].includes(f.kind) ? "border-warning/40 bg-warning/10 text-warning" : "border-line text-faint"}`}>{f.detail}</p>
                    ))}
                    {c.scripts.length > 0 && <details className="mt-1 text-[11px] text-faint"><summary className="cursor-pointer">Script files</summary><ul className="mt-1 font-mono">{c.scripts.map((s) => <li key={s}>{s}</li>)}</ul></details>}
                    <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-0.5 text-[11px]">
                      {c.compat.filter((x) => x.status !== "unsupported").map((x) => <li key={x.agent_id} className={tone[x.status]} title={x.note}>{mark[x.status]} {agentLabel[x.agent_id] ?? x.agent_id}</li>)}
                    </ul>
                  </div>
                </label>
              </li>
            ))}
          </ul>
          {error && <p className="mt-2 text-xs text-danger">{error}</p>}
          <div className="mt-4 flex items-center justify-between">
            <span className="text-xs text-faint">{picked.size} selected</span>
            <div className="flex gap-2">
              <button className={ghost} onClick={close}>Cancel</button>
              <button className="btn-primary rounded px-3 py-1 text-sm disabled:opacity-40" disabled={!!busy || picked.size === 0} onClick={install}>{busy ?? `Install ${picked.size || ""}`.trim()}</button>
            </div>
          </div>
        </>
      )}
    </Modal>
  );
}
