import { describe, test, expect } from "bun:test";
import { map, range, skip } from ".";

describe("range", () => {
  test("range(3)", () => {
    expect([...range(3)]).toEqual([0, 1, 2]);
  });
  test("range(0)", () => {
    expect([...range(0)]).toEqual([]);
  });
  test("range(5)", () => {
    expect([...range(5)]).toEqual([0, 1, 2, 3, 4]);
  });
});

describe("skip", () => {
  test("skip([1,2,3], 0)", () => {
    expect([...skip([1, 2, 3], 0)]).toEqual([1, 2, 3]);
  });
  test("skip([1,2,3], 1)", () => {
    expect([...skip([1, 2, 3], 1)]).toEqual([2, 3]);
  });
  test("skip([1,2,3], 2)", () => {
    expect([...skip([1, 2, 3], 2)]).toEqual([3]);
  });
  test("skip([1,2,3], 3)", () => {
    expect([...skip([1, 2, 3], 3)]).toEqual([]);
  });
  test("skip([1,2,3], 4)", () => {
    expect([...skip([1, 2, 3], 4)]).toEqual([]);
  });
  test("skip([], 0)", () => {
    expect([...skip([], 0)]).toEqual([]);
  });
  test("skip([], 1)", () => {
    expect([...skip([], 1)]).toEqual([]);
  });
  test("skip(range(5), 2)", () => {
    expect([...skip(range(5), 2)]).toEqual([2, 3, 4]);
  });
});

describe("map", () => {
  test("map([1, 2, 3], x => x * 2)", () => {
    expect([...map([1, 2, 3], (x) => x * 2)]).toEqual([2, 4, 6]);
  });
  test("map([], x => x * 2)", () => {
    expect([...map([], (x) => x * 2)]).toEqual([]);
  });
  test("map(range(3), x => x * 2)", () => {
    expect([...map(range(3), (x) => x * 2)]).toEqual([0, 2, 4]);
  });
});
