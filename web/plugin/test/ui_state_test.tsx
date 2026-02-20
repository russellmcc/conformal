import { z } from "zod";
import Provider from "../src/stores_provider";
import { codecFromZod, useUiStateAtom } from "../src/ui_state";
import { UiStateProvider } from "../src/ui_state_provider";
import { describe, test, expect } from "bun:test";
import { renderHook, waitFor } from "@testing-library/react";
import { ReactNode } from "react";
import { useAtom } from "jotai";
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

const useUiState = () => {
  const atom = useUiStateAtom<TestData>();
  const [state, setState] = useAtom(atom);
  return { state, setState };
};

describe("useUiStateAtom", () => {
  test("starts undefined", () => {
    const { result } = renderHook(useUiState, {
      wrapper,
    });
    expect(result.current.state).toBeUndefined();
  });

  test("can set state", async () => {
    const { result } = renderHook(useUiState, {
      wrapper,
    });
    result.current.setState({ a: 1, b: "test" });
    // Wait for the state to be set
    await waitFor(() => {
      expect(result.current.state).toEqual({ a: 1, b: "test" });
    });
  });
});
