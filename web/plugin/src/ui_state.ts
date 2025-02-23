import { WritableAtom } from "jotai";
import { createContext, useContext } from "react";
import { z } from "zod";
import { decode, encode } from "@msgpack/msgpack";

export type Codec<T> = {
  /*
   * Encode a value into a binary format.
   */
  encode: (value: T) => Uint8Array;

  /*
   * Decode a value from a binary format.
   *
   * Should throw an error if the value is not deserializable.
   */
  decode: (value: Uint8Array) => T;

  default: () => T;
};

export const codecFromZod = <T>(
  schema: z.ZodType<T>,
  d: z.infer<typeof schema>,
) => ({
  encode: (value: z.infer<typeof schema>) => encode(value),
  decode: (value: Uint8Array) => schema.parse(decode(value)),
  default: () => d,
});

export type UiStateData<T> = {
  fullAtom: WritableAtom<T | undefined, [update: T], void>;
  family: <K extends keyof T>(
    key: K,
  ) => WritableAtom<T[K] | undefined, [update: T[K]], void>;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const context = createContext<any>(null);

export const useFullUiStateAtom = <T>(): WritableAtom<
  T | undefined,
  [update: T],
  void
> => {
  const state = useContext(context) as UiStateData<T>;
  return state.fullAtom;
};

export const makeUseUiStateAtom =
  <T>() =>
  <K extends keyof T>(u: K) => {
    const state = useContext(context) as UiStateData<T>;
    return state.family(u);
  };
