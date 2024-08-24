import { describe, test, expect } from "bun:test";
import { exponentialScale } from ".";

describe("exponentialScale", () => {
  const testExpFor = (fromBreak: number, toBreak: number) => {
    const scale = exponentialScale(fromBreak, toBreak);
    expect(scale.to(0)).toBeCloseTo(0);
    expect(scale.to(fromBreak / 2)).toBeLessThan(toBreak / 2);
    expect(scale.to(fromBreak)).toBeCloseTo(toBreak);
    expect(scale.to(fromBreak + (1 - fromBreak) / 2)).toBeLessThan(
      toBreak + (1 - toBreak) / 2,
    );
    expect(scale.to(1)).toBeCloseTo(1);
    expect(scale.from(0)).toBeCloseTo(0);
    expect(scale.from(toBreak)).toBeCloseTo(fromBreak);
    expect(scale.from(1)).toBeCloseTo(1);
  };

  for (const [fromBreak, toBreak] of [
    [0.5, 0.25],
    [0.5, 0.1],
    [0.5, 0.05],
    [0.25, 0.1],
    [0.75, 0.1],
    [0.75, 0.6],
    [0.75, 0.01],
    [0.9, 0.001],
  ]) {
    test(`exponentialScale(${fromBreak}, ${toBreak})`, () => {
      testExpFor(fromBreak, toBreak);
    });
  }

  const testLinFor = (fromBreak: number) => {
    const scale = exponentialScale(fromBreak, fromBreak);
    expect(scale.to(0)).toBeCloseTo(0);
    expect(scale.to(fromBreak / 2)).toBeCloseTo(fromBreak / 2);
    expect(scale.to(fromBreak)).toBeCloseTo(fromBreak);
    expect(scale.to(fromBreak + (1 - fromBreak) / 2)).toBeCloseTo(
      fromBreak + (1 - fromBreak) / 2,
    );
    expect(scale.to(1)).toBeCloseTo(1);
    expect(scale.from(0)).toBeCloseTo(0);
    expect(scale.from(fromBreak)).toBeCloseTo(fromBreak);
    expect(scale.from(1)).toBeCloseTo(1);
  };

  for (const fromBreak of [0.1, 0.5, 0.75]) {
    test(`exponentialScale(${fromBreak}, ${fromBreak})`, () => {
      testLinFor(fromBreak);
    });
  }

  const testLogFor = (fromBreak: number, toBreak: number) => {
    const scale = exponentialScale(fromBreak, toBreak);
    expect(scale.to(0)).toBeCloseTo(0);
    expect(scale.to(fromBreak / 2)).toBeGreaterThan(toBreak / 2);
    expect(scale.to(fromBreak)).toBeCloseTo(toBreak);
    expect(scale.to(fromBreak + (1 - fromBreak) / 2)).toBeGreaterThan(
      toBreak + (1 - toBreak) / 2,
    );
    expect(scale.to(1)).toBeCloseTo(1);
    expect(scale.from(0)).toBeCloseTo(0);
    expect(scale.from(toBreak)).toBeCloseTo(fromBreak);
    expect(scale.from(1)).toBeCloseTo(1);
  };
  for (const [fromBreak, toBreak] of [
    [0.25, 0.5],
    [0.1, 0.5],
    [0.05, 0.5],
    [0.1, 0.25],
    [0.1, 0.75],
    [0.6, 0.75],
    [0.01, 0.75],
    [0.001, 0.9],
  ]) {
    test(`exponentialScale(${fromBreak}, ${toBreak})`, () => {
      testLogFor(fromBreak, toBreak);
    });
  }
});
