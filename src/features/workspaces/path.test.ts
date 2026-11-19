import { expect, test } from "vitest";
import { tildePath, uniqueTitle } from "./path";

test("home folders become ~", () => {
  expect(tildePath("/Users/lion/Documents/proj")).toBe("~/Documents/proj");
  expect(tildePath("/Users/lion")).toBe("~");
  expect(tildePath("/private/tmp/x")).toBe("/private/tmp/x");
  expect(tildePath("/Users/lionel/x")).toBe("~/x");
});

test("Linux and Windows home folders become ~", () => {
  expect(tildePath("/home/nana/code/app")).toBe("~/code/app");
  expect(tildePath("/home/nana")).toBe("~");
  expect(tildePath("C:\\Users\\nana\\code")).toBe("~\\code");
  expect(tildePath("C:/Users/nana/code")).toBe("~/code");
  expect(tildePath("D:\\work\\app")).toBe("D:\\work\\app");
});

test("repeated titles are numbered", () => {
  expect(uniqueTitle("~/a", [])).toBe("~/a");
  expect(uniqueTitle("~/a", ["~/a"])).toBe("~/a (2)");
  expect(uniqueTitle("~/a", ["~/a", "~/a (2)"])).toBe("~/a (3)");
  expect(uniqueTitle("~/a", ["~/b"])).toBe("~/a");
});
