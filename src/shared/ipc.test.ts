import { describe, expect, it } from "vitest";
import { isAppError } from "./ipc";

describe("isAppError", () => {
  it("recognises the serialised Rust error shape", () => {
    expect(isAppError({ kind: "internal", message: "boom" })).toBe(true);
  });

  it("rejects plain strings and incomplete objects", () => {
    expect(isAppError("boom")).toBe(false);
    expect(isAppError({ message: "boom" })).toBe(false);
    expect(isAppError(null)).toBe(false);
  });
});
