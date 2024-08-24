import { describe, test, expect } from "bun:test";
import { clamp, indexOf } from ".";

describe("indexOf", () => {
  test("indexOf(1, [1, 2, 3])", () => {
    expect(indexOf(1, [1, 2, 3])).toBe(0);
  });
  test("indexOf(2, [1, 2, 3])", () => {
    expect(indexOf(2, [1, 2, 3])).toBe(1);
  });
  test("indexOf(4, [1, 2, 3])", () => {
    expect(indexOf(4, [1, 2, 3])).toBeUndefined();
  });
});

describe("clamp", () => {
  test("clamp(5, 0, 10)", () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });
  test("clamp(-1, 0, 10)", () => {
    expect(clamp(-1, 0, 10)).toBe(0);
  });
  test("clamp(11, 0, 10)", () => {
    expect(clamp(11, 0, 10)).toBe(10);
  });
});
