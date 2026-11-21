/**
 * pane id -> backend session id. Deliberately outside React state: it is only read in event
 * handlers (e.g. "does this pane have a running foreground job?").
 */
const sessions = new Map<string, number>();

export const sessionRegistry = {
  set: (paneId: string, sessionId: number) => void sessions.set(paneId, sessionId),
  delete: (paneId: string) => void sessions.delete(paneId),
  get: (paneId: string) => sessions.get(paneId),
  /** Backend session ids this page already has a pane for. */
  ids: (): Set<number> => new Set(sessions.values()),
};
