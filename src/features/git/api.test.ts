import { expect, test } from "vitest";
import { branchLabel, dirtyCount, groupEntries, type FileEntry } from "./api";

const e = (path: string, kind: FileEntry["kind"], index: string | null, worktree: string | null): FileEntry => ({ path, orig_path: null, kind, index, worktree, submodule: false });

test("a file changed in both places appears in both groups, untracked and conflicts are separate", () => {
  const g = groupEntries([e("mm", "changed", "M", "M"), e("s", "changed", "A", null), e("u", "changed", null, "M"), e("x", "untracked", null, "?"), e("c", "unmerged", "U", "U"), e("r", "renamed", "R", null)]);
  expect(g.staged.map((f) => f.path)).toEqual(["mm", "s", "r"]);
  expect(g.unstaged.map((f) => f.path)).toEqual(["mm", "u"]);
  expect(g.untracked.map((f) => f.path)).toEqual(["x"]);
  expect(g.conflicts.map((f) => f.path)).toEqual(["c"]);
});

test("branch labels cover normal, detached and brand-new repositories", () => {
  expect(branchLabel({ branch: "main", detached: false, head_oid: "abc" })).toBe("main");
  expect(branchLabel({ branch: null, detached: true, head_oid: "1234567890" })).toBe("HEAD detached at 1234567");
  expect(branchLabel({ branch: null, detached: false, head_oid: null })).toBe("(no commits yet)");
});

test("dirtyCount sums every kind of change", () => {
  expect(dirtyCount({ counts: { staged: 1, unstaged: 2, untracked: 3, conflicted: 4 } })).toBe(10);
  expect(dirtyCount({ counts: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 } })).toBe(0);
});
