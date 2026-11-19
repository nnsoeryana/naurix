import { expect, test } from "vitest";
import { tildePath, uniqueTitle } from "./path";

test("home folders become ~", () => {
  expect(tildePath("/Users/lion/Documents/proj")).toBe("~/Documents/proj");
  expect(tildePath("/Users/lion")).toBe("~");
  expect(tildePath("/private/tmp/x")).toBe("/private/tmp/x");
  expect(tildePath("/Users/lionel/x")).toBe("~/x");
});

test("repeated titles are numbered", () => {
  expect(uniqueTitle("~/a", [])).toBe("~/a");
  expect(uniqueTitle("~/a", ["~/a"])).toBe("~/a (2)");
  expect(uniqueTitle("~/a", ["~/a", "~/a (2)"])).toBe("~/a (3)");
  expect(uniqueTitle("~/a", ["~/b"])).toBe("~/a");
});
