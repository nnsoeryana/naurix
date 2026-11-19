import { invoke } from "@tauri-apps/api/core";
import { useStore } from "../../state/store";
import { terminalHandles } from "./handles";

const fail = (e: unknown) => useStore.getState().setNotice(String(e));

/** Copy the pane's selection to the system clipboard. Only runs from an explicit user action. */
export function copyPane(paneId: string): void {
  const handle = terminalHandles.get(paneId);
  const text = handle?.getSelection() ?? "";
  if (text) invoke("clipboard_write", { text }).catch(fail);
  handle?.focus();
}

/** Paste the system clipboard into the pane (bracketed paste is decided by xterm). */
export function pastePane(paneId: string): void {
  const handle = terminalHandles.get(paneId);
  void invoke<string>("clipboard_read")
    .then((text) => {
      if (text) handle?.paste(text);
      handle?.focus();
    })
    .catch(fail);
}
