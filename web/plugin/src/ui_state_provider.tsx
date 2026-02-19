import { Codec, context, UiStateData } from "./ui_state";
import { atom } from "jotai";
import { useStores } from "./stores_react";
import { ReactNode } from "react";

/**
 * Provides the ui state to the component tree.
 *
 * Use the `useUiStateAtom` hook to get the atom for the ui state.
 *
 * Note that this requires a `Provider` to be present to use the store.
 */
export const UiStateProvider = <T,>({
  codec,
  children,
}: {
  codec: Codec<T>;
  children: ReactNode;
}) => {
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
