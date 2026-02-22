import { z } from "zod";
import Provider from "../src/stores_provider";
import { codecFromZod, useUiState } from "../src/ui_state";
import { UiStateProvider } from "../src/ui_state_provider";
import { describe, test, expect } from "bun:test";
import { renderHook, waitFor } from "@testing-library/react";
import { ReactNode } from "react";
const testSchema = z.object({
  a: z.number(),
  b: z.string(),
});

type TestData = z.infer<typeof testSchema>;

const wrapper = ({ children }: { children: ReactNode }) => (
  <Provider mockInfos={new Map()}>
    <UiStateProvider codec={codecFromZod(testSchema)}>
      {children}
    </UiStateProvider>
  </Provider>
);

describe("useUiStateAtom", () => {
  test("starts undefined", () => {
    const { result } = renderHook(useUiState<TestData>, {
      wrapper,
    });
    expect(result.current.value).toBeUndefined();
  });

  test("can set state", async () => {
    const { result } = renderHook(useUiState, {
      wrapper,
    });
    result.current.set({ a: 1, b: "test" });
    // Wait for the state to be set
    await waitFor(() => {
      expect(result.current.value).toEqual({ a: 1, b: "test" });
    });
  });
});
