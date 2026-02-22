import { Codec, context, UiStateData } from "./ui_state";
import { atom } from "jotai";
import { useStores } from "./stores_react";
import { ReactNode } from "react";

/**
 * Props for the {@link UiStateProvider} component.
 *
 * @typeParam T - The type of the ui state.
 * @group Component Props
 */
export type UiStateProviderProps<T> = {
  /** The codec to use to encode and decode the ui state */
  codec: Codec<T>;
  /** @hidden */
  children: ReactNode;
};

/**
 * Provides the ui state to the component tree.
 *
 * Use the {@link useUiState} hook to get and set the UI State.
 *
 * UI State is arbitrary data managed by the react UI that is not connected
 * to any plug-in parameters. This data is saved in the DAW alongside the plug-in,
 * and persists between DAW sessions.
 *
 * Note that this requires a {@link Provider} to be present to use the store.
 *
 * @typeParam T - The type of the ui state.
 * @group Components
 */
export const UiStateProvider = <T,>({
  codec,
  children,
}: UiStateProviderProps<T>) => {
  const rawState = useStores().bytes("ui-state");
  const stateAtom = atom(
    (get) => {
      const raw = get(rawState);
      if (raw instanceof Promise) {
        return undefined;
      }
      try {
        return codec.decode(raw);
      } catch {
        return undefined;
      }
    },
    (_get, set, update: T) => {
      const raw = codec.encode(update);
      set(rawState, raw);
    },
  );
  const state: UiStateData<T> = { atom: stateAtom };
  return <context.Provider value={state}>{children}</context.Provider>;
};
