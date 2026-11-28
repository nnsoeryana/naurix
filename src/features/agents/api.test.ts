import { describe, expect, it } from "vitest";
import { parseExitCodes } from "./api";

describe("success exit codes (ADR 0032)", () => {
  it("reads commas and spaces, and empty means the default", () => {
    expect(parseExitCodes("0, 1")).toEqual([0, 1]);
    expect(parseExitCodes(" 0 1 ")).toEqual([0, 1]);
    expect(parseExitCodes("")).toEqual([0]);
  });

  it("refuses anything that is not a whole number from 0 to 255", () => {
    for (const bad of ["256", "-1", "1.5", "a", "0x1", "1000"]) expect(parseExitCodes(bad), bad).toBeNull();
  });
});
