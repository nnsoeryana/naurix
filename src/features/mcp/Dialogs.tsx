import { useState } from "react";
import { summarize, type CatalogEntry, type McpInput, type McpServer, type Pair } from "./api";

const field = "w-full rounded border border-line bg-midnight px-2 py-1 text-sm outline-none focus:border-purple";
const btn = "rounded px-3 py-1 text-sm";
const ghost = `${btn} bg-raised hover:bg-line`;

function Modal({ title, onClose, children, wide }: { title: string; onClose: () => void; children: React.ReactNode; wide?: boolean }) {
  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-auto bg-overlay py-16" onMouseDown={onClose}>
      <div className={`${wide ? "w-[40rem]" : "w-[32rem]"} rounded-lg border border-line bg-dialog p-4 shadow-xl`} role="dialog" aria-modal="true" aria-label={title} onMouseDown={(e) => e.stopPropagation()} onKeyDown={(e) => e.key === "Escape" && onClose()}>
        <h2 className="mb-3 text-sm font-semibold">{title}</h2>
        {children}
      </div>
    </div>
  );
}

export function CatalogDialog({ entries, onPick, onClose }: { entries: CatalogEntry[]; onPick: (e: CatalogEntry) => void; onClose: () => void }) {
  return (
    <Modal title="Add from catalog" onClose={onClose} wide>
      <p className="mb-3 text-xs text-muted">Well-known servers, with settings taken from each project&apos;s own documentation. Nothing runs until you review and approve it.</p>
      <ul className="space-y-2">
        {entries.map((e) => (
          <li key={e.id} className="rounded-lg border border-line p-3">
            <div className="flex items-start gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 text-sm font-medium">{e.title}<span className="rounded bg-raised px-1.5 text-[10px] uppercase text-muted">{e.transport === "stdio" ? "local" : "remote"}</span></div>
                <p className="text-xs text-muted">{e.description}</p>
                <ul className="mt-1 list-disc pl-4 text-[11px] text-faint">{e.notes.map((n) => <li key={n}>{n}</li>)}</ul>
                <a className="text-[11px] text-purple underline" href={e.source} target="_blank" rel="noreferrer noopener">Source</a>
              </div>
              <button className="btn-primary shrink-0 rounded px-3 py-1 text-xs" onClick={() => onPick(e)}>Add…</button>
            </div>
          </li>
        ))}
      </ul>
      <div className="mt-3 flex justify-end"><button className={ghost} onClick={onClose}>Close</button></div>
    </Modal>
  );
}

const lines = (s: string) => s.split("\n").map((l) => l.trimEnd()).filter((l) => l !== "");
const toPairs = (s: string): Pair[] => lines(s).map((l) => { const i = l.indexOf("="); return i < 0 ? { name: l.trim(), value: "" } : { name: l.slice(0, i).trim(), value: l.slice(i + 1) }; });
const fromPairs = (p: Pair[], sep: string) => p.map((x) => `${x.name}${sep}${x.value}`).join("\n");
const toHeaders = (s: string): Pair[] => lines(s).map((l) => { const i = l.indexOf(":"); return i < 0 ? { name: l.trim(), value: "" } : { name: l.slice(0, i).trim(), value: l.slice(i + 1).trim() }; });

export function EditorDialog({ initial, isNew, onSave, onClose }: { initial: McpInput; isNew: boolean; onSave: (i: McpInput) => Promise<void>; onClose: () => void }) {
  const [name, setName] = useState(initial.name);
  const [transport, setTransport] = useState(initial.transport);
  const [command, setCommand] = useState(initial.command ?? "");
  const [args, setArgs] = useState(initial.args.join("\n"));
  const [url, setUrl] = useState(initial.url ?? "");
  const [env, setEnv] = useState(fromPairs(initial.env, "="));
  const [headers, setHeaders] = useState(fromPairs(initial.headers, ": "));
  const [secrets, setSecrets] = useState(initial.secrets.join(", "));
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const input = (): McpInput => ({
    id: initial.id ?? null,
    name: name.trim(),
    transport,
    command: transport === "stdio" ? command.trim() : null,
    args: transport === "stdio" ? lines(args) : [],
    url: transport === "http" ? url.trim() : null,
    env: transport === "stdio" ? toPairs(env) : [],
    headers: transport === "http" ? toHeaders(headers) : [],
    secrets: secrets.split(",").map((s) => s.trim()).filter(Boolean),
  });

  return (
    <Modal title={isNew ? "Add MCP server" : `Edit “${initial.name}”`} onClose={onClose} wide>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          setBusy(true);
          onSave(input())
            .catch((err) => setError(String(err)))
            .finally(() => setBusy(false));
        }}
      >
        <div className="grid grid-cols-[1fr_9rem] gap-3">
          <label className="text-xs text-faint">Name<input className={`${field} mt-1`} value={name} maxLength={64} onChange={(e) => setName(e.target.value)} /></label>
          <label className="text-xs text-faint">Type
            <select className={`${field} mt-1`} value={transport} onChange={(e) => setTransport(e.target.value as "stdio" | "http")}>
              <option value="stdio">Local (stdio)</option>
              <option value="http">Remote (HTTP)</option>
            </select>
          </label>
        </div>
        {transport === "stdio" ? (
          <>
            <label className="mt-3 block text-xs text-faint">Command (a name like <code>npx</code>, or an absolute path)<input className={`${field} mt-1 font-mono`} value={command} onChange={(e) => setCommand(e.target.value)} /></label>
            <label className="mt-3 block text-xs text-faint">Arguments, one per line<textarea className={`${field} mt-1 h-20 font-mono`} value={args} onChange={(e) => setArgs(e.target.value)} /></label>
            <label className="mt-3 block text-xs text-faint">Environment, <code>NAME=value</code> per line<textarea className={`${field} mt-1 h-16 font-mono`} value={env} onChange={(e) => setEnv(e.target.value)} /></label>
          </>
        ) : (
          <>
            <label className="mt-3 block text-xs text-faint">URL<input className={`${field} mt-1 font-mono`} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://…" /></label>
            <label className="mt-3 block text-xs text-faint">Headers, <code>Name: value</code> per line<textarea className={`${field} mt-1 h-16 font-mono`} value={headers} onChange={(e) => setHeaders(e.target.value)} /></label>
          </>
        )}
        <label className="mt-3 block text-xs text-faint">Secrets (comma-separated, UPPER_CASE)
          <input className={`${field} mt-1 font-mono`} value={secrets} onChange={(e) => setSecrets(e.target.value)} placeholder="GITHUB_PAT" />
        </label>
        <p className="mt-1 text-[11px] text-faint">
          Never type a token or password into the fields above. Declare a secret, write <code>{"${secret:NAME}"}</code> where it belongs (for example <code>Authorization: Bearer {"${secret:GITHUB_PAT}"}</code>), then set its value on the server card. The value is kept in the macOS Keychain.
        </p>
        {error && <p className="mt-2 text-xs text-danger">{error}</p>}
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className={ghost} onClick={onClose}>Cancel</button>
          <button type="submit" disabled={busy} className={`${btn} btn-primary disabled:opacity-50`}>{isNew ? "Add server" : "Save"}</button>
        </div>
      </form>
    </Modal>
  );
}

/** Shows exactly what would run, and binds approval to that definition's fingerprint. */
export function ApprovalDialog({ server, onApprove, onClose }: { server: McpServer; onApprove: () => void; onClose: () => void }) {
  return (
    <Modal title={`Review “${server.name}”`} onClose={onClose} wide>
      <p className="mb-2 text-xs text-muted">{server.transport === "stdio" ? "Approving lets Naurix run this command when you press Test connection, and write it into agent config files when you sync." : "Approving lets Naurix write this remote server into agent config files when you sync."} Nothing runs on its own, and changing any of the details below asks for approval again.</p>
      <pre className="mb-3 overflow-auto rounded bg-midnight p-3 font-mono text-xs text-cloud" data-approval-command>{summarize(server)}</pre>
      <dl className="mb-3 space-y-1 text-xs text-muted">
        {server.env.length > 0 && <div><dt className="inline text-faint">Environment: </dt><dd className="inline font-mono">{server.env.map((e) => `${e.name}=${e.value}`).join(", ")}</dd></div>}
        {server.headers.length > 0 && <div><dt className="inline text-faint">Headers: </dt><dd className="inline font-mono">{server.headers.map((h) => `${h.name}: ${h.value}`).join(", ")}</dd></div>}
        {server.secrets.length > 0 && <div><dt className="inline text-faint">Secrets (values stay in the Keychain): </dt><dd className="inline font-mono">{server.secrets.map((s) => s.name).join(", ")}</dd></div>}
      </dl>
      {server.command && ["npx", "bunx", "pnpm", "uvx", "pipx"].some((c) => server.command === c || server.command?.endsWith(`/${c}`)) && (
        <p className="mb-3 rounded border border-warning/40 bg-warning/10 p-2 text-xs text-warning">This command downloads and runs a package from the internet. Only approve servers you trust.</p>
      )}
      <div className="flex justify-end gap-2">
        <button className={ghost} onClick={onClose}>Cancel</button>
        <button className={`${btn} btn-primary`} onClick={onApprove}>Approve</button>
      </div>
    </Modal>
  );
}
