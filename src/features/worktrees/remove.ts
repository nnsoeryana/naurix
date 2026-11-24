import { useStore } from "../../state/store";
import { approvalApi } from "../approvals/api";
import { worktreeApi, type ManagedWorktree } from "./api";

/**
 * Remove a worktree Naurix created, in the order the approval gate requires: the backend checks it can
 * be removed and builds the review, the user confirms exactly that text, then the backend removes it
 * (without --force; the branch is kept). A worktree that cannot be removed shows the reason instead.
 */
export function startRemove(w: ManagedWorktree, done: () => void): void {
  const { setDialog, setNotice } = useStore.getState();
  worktreeApi
    .reviewRemove(w.id)
    .then((request) =>
      setDialog({
        kind: "confirm",
        title: "Remove this worktree?",
        message: request.display,
        confirmLabel: "Allow once and remove",
        danger: true,
        onConfirm: () => {
          approvalApi
            .decide(request.id, { decision: "allow", scope: "once" })
            .then(() => worktreeApi.remove(w.id))
            .then(done)
            .catch((e) => setNotice(String(e)));
        },
      }),
    )
    .catch((e) => setNotice(String(e)));
}

/** The folder was deleted by hand: forget Naurix's record. No Git change, so no approval. */
export function forgetMissing(w: ManagedWorktree, done: () => void): void {
  const { setNotice } = useStore.getState();
  worktreeApi.forget(w.id).then(done).catch((e) => setNotice(String(e)));
}
