import { renderHook } from "@testing-library/react";
import { describe, expect, test } from "bun:test";
import useAnimation from "./useAnimation";

interface TestState {
  value: number;
}
interface TestData {
  forward: boolean;
}

const customTestAnimationForLimit = (limit: number) => ({
  initialState: () => ({
    value: 0,
  }),
  update: (
    elapsed: number | undefined,
    prev: TestState,
    data: TestData,
  ): TestState => ({
    value: Math.min(
      Math.max(prev.value + (data.forward ? 1 : -1) * (elapsed ?? 0), -limit),
      limit,
    ),
  }),
  shouldAnimate: (state: TestState, data: TestData) =>
    state.value !== (data.forward ? 1 : -1) * limit,
});

const limit = 0.001;
const customTestAnimation = customTestAnimationForLimit(limit);
const customTestAnimation2 = customTestAnimationForLimit(2 * limit);

const until = async (
  c: () => boolean,
  rate: number,
  timeout: number,
): Promise<boolean> => {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (c()) {
      return true;
    }
    await new Promise((r) => setTimeout(r, rate));
  }
  return false;
};

describe("useAnimation", () => {
  test("switch directions", async () => {
    const { result, rerender } = renderHook(
      ({ data }) => useAnimation(customTestAnimation, data),
      { initialProps: { data: { forward: true } } },
    );
    expect(result.current).toEqual({ value: 0 });
    expect(await until(() => result.current.value === limit, 1, 100)).toBe(
      true,
    );
    expect(result.current).toEqual({ value: limit });

    rerender({ data: { forward: false } });
    expect(await until(() => result.current.value === -limit, 1, 1000)).toBe(
      true,
    );
  });
  test("switch limit", async () => {
    const { result, rerender } = renderHook(
      ({ ca }) => useAnimation(ca, { forward: true }),
      { initialProps: { ca: customTestAnimation } },
    );
    expect(result.current).toEqual({ value: 0 });
    expect(await until(() => result.current.value === limit, 1, 100)).toBe(
      true,
    );
    expect(result.current).toEqual({ value: limit });
    rerender({ ca: customTestAnimation2 });
    expect(await until(() => result.current.value === 2 * limit, 1, 1000)).toBe(
      true,
    );
  });
});
