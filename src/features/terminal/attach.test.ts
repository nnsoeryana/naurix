import { expect, test } from "vitest";
import { clipboardHasOnlyImage, pathsToPaste, quotePath } from "./attach";

test("plain paths stay readable, others are single-quoted on Unix", () => {
  expect(quotePath("/Users/me/shot.png", false)).toBe("/Users/me/shot.png");
  expect(quotePath("/Users/me/My Shot.png", false)).toBe("'/Users/me/My Shot.png'");
  expect(quotePath("/tmp/it's.png", false)).toBe(`'/tmp/it'\\''s.png'`);
  expect(quotePath("/tmp/$(rm -rf x).png", false)).toBe("'/tmp/$(rm -rf x).png'");
});

test("Windows paths are double-quoted only when needed", () => {
  expect(quotePath("C:\\Users\\me\\a.png", true)).toBe("C:\\Users\\me\\a.png");
  expect(quotePath("C:\\Users\\me\\My Shot.png", true)).toBe('"C:\\Users\\me\\My Shot.png"');
});

test("dropped paths become one pasteable line with a trailing space and no newline", () => {
  const t = pathsToPaste(["/a/b.png", "/a/c d.png"], false);
  expect(t).toBe("/a/b.png '/a/c d.png' ");
  expect(t).not.toMatch(/[\r\n]/);
});

test("paths with control characters are skipped, and the count is bounded", () => {
  expect(pathsToPaste(["/a/evil\n.png", "/a/ok.png", "/a/\x1b[31m"], false)).toBe("/a/ok.png ");
  expect(pathsToPaste(["/a/evil\n.png"], false)).toBe("");
  expect(pathsToPaste([], false)).toBe("");
  expect(pathsToPaste(Array.from({ length: 50 }, (_, i) => `/a/${i}.png`), false).trim().split(" ")).toHaveLength(20);
});

const dt = (types: string[], text: string, items: { kind: string; type: string }[]) =>
  ({ types, getData: () => text, items } as unknown as Parameters<typeof clipboardHasOnlyImage>[0]);

test("a screenshot (image, no text) is detected; text or file-name pastes are not", () => {
  expect(clipboardHasOnlyImage(dt(["Files"], "", [{ kind: "file", type: "image/png" }]))).toBe(true);
  expect(clipboardHasOnlyImage(dt(["text/plain", "Files"], "photo.png", [{ kind: "file", type: "image/png" }]))).toBe(false);
  expect(clipboardHasOnlyImage(dt(["text/plain"], "hello", [{ kind: "string", type: "text/plain" }]))).toBe(false);
  expect(clipboardHasOnlyImage(dt(["Files"], "", [{ kind: "file", type: "application/pdf" }]))).toBe(false);
  expect(clipboardHasOnlyImage(null)).toBe(false);
});
