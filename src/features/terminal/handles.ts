/**
 * pane id -> imperative handle of its terminal (for the context menu). Kept outside React state:
 * terminal contents and selections never enter the store.
 */
export type TerminalHandle = {
  hasSelection: () => boolean;
  getSelection: () => string;
  /** Paste with bracketed-paste handling (xterm decides based on the app's mode). */
  paste: (text: string) => void;
  focus: () => void;
};

const handles = new Map<string, TerminalHandle>();

export const terminalHandles = {
  set: (paneId: string, h: TerminalHandle) => void handles.set(paneId, h),
  delete: (paneId: string) => void handles.delete(paneId),
  get: (paneId: string) => handles.get(paneId),
};
