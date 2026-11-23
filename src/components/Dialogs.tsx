import { useEffect, useRef, useState } from "react";
import { useStore, type Dialog } from "../state/store";

const btn = "rounded px-3 py-1 text-sm";

export function DialogHost() {
  const dialog = useStore((s) => s.dialog);
  const setDialog = useStore((s) => s.setDialog);
  if (!dialog) return null;
  const close = () => setDialog(null);
  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-overlay pt-32" onMouseDown={close}>
      <div className="w-[26rem] rounded-lg border border-line bg-dialog p-4 shadow-xl" role="dialog" aria-modal="true" onMouseDown={(e) => e.stopPropagation()}>
        {dialog.kind === "confirm" ? <Confirm d={dialog} close={close} /> : <Prompt d={dialog} close={close} />}
      </div>
    </div>
  );
}

function Confirm({ d, close }: { d: Extract<Dialog, { kind: "confirm" }>; close: () => void }) {
  const ok = useRef<HTMLButtonElement>(null);
  useEffect(() => ok.current?.focus(), []);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        close();
        d.onConfirm();
      }}
      onKeyDown={(e) => e.key === "Escape" && close()}
    >
      <h2 className="mb-1 text-sm font-semibold">{d.title}</h2>
      <p className="mb-4 whitespace-pre-wrap break-words text-sm text-muted">{d.message}</p>
      <div className="flex justify-end gap-2">
        <button type="button" className={`${btn} bg-raised hover:bg-line`} onClick={close}>Cancel</button>
        <button ref={ok} type="submit" className={`${btn} ${d.danger ? "btn-danger" : "btn-primary"}`}>{d.confirmLabel}</button>
      </div>
    </form>
  );
}

function Prompt({ d, close }: { d: Extract<Dialog, { kind: "prompt" }>; close: () => void }) {
  const [value, setValue] = useState(d.initial);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.select(), []);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (value.trim()) {
          close();
          d.onSubmit(value);
        }
      }}
      onKeyDown={(e) => e.key === "Escape" && close()}
    >
      <h2 className="mb-2 text-sm font-semibold">{d.title}</h2>
      <input ref={input} autoFocus value={value} maxLength={100} onChange={(e) => setValue(e.target.value)} className="mb-4 w-full rounded border border-line bg-midnight px-2 py-1 text-sm outline-none focus:border-purple" />
      <div className="flex justify-end gap-2">
        <button type="button" className={`${btn} bg-raised hover:bg-line`} onClick={close}>Cancel</button>
        <button type="submit" className={`${btn} btn-primary`}>Save</button>
      </div>
    </form>
  );
}
