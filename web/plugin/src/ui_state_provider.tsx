import { Codec, context, UiStateData } from "./ui_state";
import { atom, WritableAtom } from "jotai";
import { useStores } from "./stores_react";
import { atomFamily } from "jotai/utils";

export const UiStateProvider = <T,>({
  codec,
  children,
}: {
  codec: Codec<T>;
  children: React.ReactNode;
}) => {
  const rawState = useStores().bytes("ui-state");
  const fullAtom = atom(
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
  const family = atomFamily((key: keyof T) =>
    atom(
      (get) => {
        const full = get(fullAtom);
        return full?.[key];
      },
      (get, set, update: T[keyof T]) => {
        const full = get(fullAtom);
        if (full) {
          // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-explicit-any
          full[key] = update as unknown as any;
          set(fullAtom, full);
        } else {
          const d = codec.default();
          d[key] = update;
          set(fullAtom, d);
        }
      },
    ),
  ) as <K extends keyof T>(
    key: K,
  ) => WritableAtom<T[K] | undefined, [update: T[K]], void>;
  const state: UiStateData<T> = { fullAtom, family };
  return <context.Provider value={state}>{children}</context.Provider>;
};
