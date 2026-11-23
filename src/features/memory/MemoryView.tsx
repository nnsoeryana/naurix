import { useCallback, useEffect, useRef, useState } from "react";
import { activeSpace, useStore } from "../../state/store";
import { terminalHandles } from "../terminal/handles";
import { MAX_BODY, MAX_TITLE, memoryApi, type MemoryStatus, type Note } from "./api";
import { insertTarget, insertText, pasteRefusal } from "./model";

const outline = "rounded border border-line px-2 py-0.5 text-xs text-muted hover:bg-raised disabled:opacity-40";
const input = "w-full rounded border border-line bg-transparent px-2 py-1 text-sm";

type Edit = { id: string | null; title: string; body: string };

/** Project memory (ADR 0031): notes the user writes, per workspace, off until switched on. */
export function MemoryView() {
  const wsId = useStore((s) => s.activeWorkspaceId);
  const workspace = useStore((s) => s.workspaces.find((w) => w.id === s.activeWorkspaceId));
  const confirm = useStore((s) => s.setDialog);
  const setView = useStore((s) => s.setView);
  const [status, setStatus] = useState<MemoryStatus | null>(null);
  const [notes, setNotes] = useState<Note[]>([]);
  const [query, setQuery] = useState("");
  const [edit, setEdit] = useState<Edit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  // Every load gets a number; an answer that is no longer the latest is dropped, so a slow reply for an
  // older query cannot overwrite the results of the one the user typed last.
  const latest = useRef(0);
  const load = useCallback(async () => {
    if (!wsId) return;
    const mine = ++latest.current;
    try {
      const s = await memoryApi.status(wsId);
      const list = s.enabled ? await memoryApi.list(wsId, query.trim() || null) : [];
      if (mine !== latest.current) return;
      setStatus(s);
      setNotes(list);
    } catch (e) {
      if (mine === latest.current) setError(String(e));
    }
  }, [wsId, query]);
  useEffect(() => void load(), [load]);
  // A draft belongs to the project it was started in.
  useEffect(() => {
    setEdit(null);
    setError(null);
    setInfo(null);
  }, [wsId]);

  if (!wsId || !workspace) return <div className="p-6 text-sm text-muted">Open a project to use memory.</div>;
  const run = (p: Promise<unknown>, after?: () => void) => void p.then(() => (setError(null), after?.(), load())).catch((e) => setError(String(e)));

  const save = async () => {
    if (!edit) return;
    try {
      await memoryApi.save(wsId, edit.id, edit.title, edit.body);
      setEdit(null);
      setError(null);
      await load();
    } catch (e) {
      setError(String(e)); // a refused secret names the line and kind, never the value
    }
  };

  const insert = (n: Note) => {
    const s = useStore.getState();
    const sp = activeSpace(s);
    const tab = sp?.tabs.find((t) => t.id === sp.activeTabId);
    const t = insertTarget(tab?.activePaneId, s.paneKinds, s.paneState);
    if (!t.ok) return setError(t.reason);
    const handle = terminalHandles.get(t.paneId);
    const text = insertText(n);
    const refusal = pasteRefusal(text, handle?.bracketedPaste() ?? false);
    if (refusal) return setError(refusal);
    handle?.paste(text);
    setError(null);
    setView("terminal");
    terminalHandles.get(t.paneId)?.focus();
  };

  const exportAs = (format: "markdown" | "json") =>
    void memoryApi
      .export(wsId, format)
      .then((r) => setInfo(r.kind === "cancelled" ? "Nothing was written." : `Saved to ${r.path}${r.inRepository ? " (inside a Git repository, as you confirmed)" : ""}.`))
      .catch((e) => setError(String(e)));

  return (
    <div className="h-full overflow-auto p-6" data-memory>
      <div className="mx-auto max-w-3xl space-y-4">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-semibold">Memory</h1>
          <span className="text-xs text-faint">{workspace.name}</span>
        </div>
        <p className="text-xs text-muted">
          Notes you write for this project, kept on this computer only and off until you switch them on. Naurix never adds anything by itself: no prompts, no terminal output, no files.
          Text that looks like a password, token or key is refused, but that check is a safety net, not a guarantee. Keep secrets in your password manager.
          Notes are never written into your project or committed.
        </p>
        {error && <p className="rounded border border-danger/40 p-2 text-xs text-danger" role="alert" data-memory-error>{error}</p>}
        {info && <p className="text-xs text-teal" data-memory-info>{info}</p>}

        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={!!status?.enabled} onChange={(e) => run(memoryApi.setEnabled(wsId, e.target.checked).then(setStatus), () => setEdit(null))} data-memory-toggle />
          Use memory in this project
        </label>

        {status && !status.enabled && (
          <div className="space-y-2 text-xs text-muted" data-memory-off>
            <p>Memory is off. {status.count > 0 ? `${status.count} saved ${status.count === 1 ? "note is" : "notes are"} kept and hidden.` : "Nothing is stored."}</p>
            {status.count > 0 && (
              <button className={outline} onClick={() => confirm({ kind: "confirm", title: "Delete all notes?", message: `Delete the ${status.count} hidden notes of ${workspace.name}? This cannot be undone.`, confirmLabel: "Delete all", danger: true, onConfirm: () => run(memoryApi.removeAll(wsId)) })}>Delete all notes</button>
            )}
          </div>
        )}

        {status?.enabled && (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <input aria-label="Search notes" className={`${input} max-w-xs`} placeholder="Search notes" value={query} onChange={(e) => setQuery(e.target.value)} />
              <button className="btn-primary rounded-lg px-3 py-1 text-xs" disabled={!!edit} onClick={() => setEdit({ id: null, title: "", body: "" })} data-memory-new>New note</button>
              <span className="ml-auto flex gap-2">
                <button className={outline} disabled={status.count === 0} onClick={() => exportAs("markdown")}>Export Markdown</button>
                <button className={outline} disabled={status.count === 0} onClick={() => exportAs("json")}>Export JSON</button>
                <button className={outline} disabled={status.count === 0} onClick={() => confirm({ kind: "confirm", title: "Delete all notes?", message: `Delete all ${status.count} notes of ${workspace.name}? This cannot be undone.`, confirmLabel: "Delete all", danger: true, onConfirm: () => run(memoryApi.removeAll(wsId)) })}>Delete all</button>
              </span>
            </div>

            {edit && (
              <section className="space-y-2 rounded-lg border border-line p-3" data-memory-editor>
                <input aria-label="Note title" className={input} placeholder="Title" maxLength={MAX_TITLE} value={edit.title} onChange={(e) => setEdit({ ...edit, title: e.target.value })} />
                <textarea aria-label="Note text" className={`${input} h-40 font-mono text-xs`} placeholder="What should be remembered about this project?" maxLength={MAX_BODY} value={edit.body} onChange={(e) => setEdit({ ...edit, body: e.target.value })} />
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-faint">{edit.body.length}/{MAX_BODY}</span>
                  <button className="btn-primary ml-auto rounded-lg px-3 py-1 text-xs" onClick={() => void save()} data-memory-save>Save</button>
                  <button className={outline} onClick={() => (setEdit(null), setError(null))}>Cancel</button>
                </div>
              </section>
            )}

            <ul className="space-y-2">
              {notes.length === 0 && !edit && <li className="text-sm text-faint">{query.trim() ? "No notes match." : "No notes yet."}</li>}
              {notes.map((n) => (
                <li key={n.id} className="space-y-1 rounded-lg border border-line px-3 py-2" data-note>
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm text-cloud">{n.title}</span>
                    <button className={outline} onClick={() => insert(n)} title="Types this note into the active agent's input. Enter is not pressed.">Insert into agent terminal</button>
                    <button className={outline} onClick={() => (setEdit({ id: n.id, title: n.title, body: n.body }), setError(null))}>Edit</button>
                    <button className={outline} onClick={() => run(memoryApi.remove(wsId, n.id))}>Delete</button>
                  </div>
                  {n.body && <pre className="max-h-40 overflow-auto whitespace-pre-wrap font-mono text-[11px] text-muted">{n.body}</pre>}
                  <div className="text-[10px] text-faint">Updated {n.updatedAt}</div>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
