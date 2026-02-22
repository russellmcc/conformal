import { useAtom, WritableAtom } from "jotai";
import { createContext, useContext } from "react";
import { z } from "zod";
import { decode, encode } from "@msgpack/msgpack";

/**
 * A `Codec` for a type `T` defines how to serialize and deserialize
 * the value to and from a binary format.
 *
 * @typeParam T - The type of the value to encode and decode.
 * @group Types
 */
export type Codec<T> = {
  /**
   * Encode a value into a binary format.
   */
  encode: (value: T) => Uint8Array;

  /**
   * Decode a value from a binary format.
   *
   * Should throw an error if the value is not deserializable.
   */
  decode: (value: Uint8Array) => T;
};

export const codecFromZod = <T>(schema: z.ZodType<T>) => ({
  encode: (value: z.infer<typeof schema>) => encode(value),
  decode: (value: Uint8Array) => schema.parse(decode(value)),
});

export type UiStateData<T> = {
  atom: WritableAtom<T | undefined, [update: T], void>;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const context = createContext<any>(null);

/**
 * Gets the atom for the ui state. Note that the type must match the type
 * of the state in the UiStateProvider.
 */
const useUiStateAtom = <T>(): WritableAtom<
  T | undefined,
  [update: T],
  void
> => {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion
  const state = useContext(context) as UiStateData<T>;
  return state.atom;
};

/**
 * @group Hooks
 */
export const useUiState = <T>(): {
  value: T | undefined;
  set: (update: T) => void;
} => {
  const atom = useUiStateAtom<T>();
  const [value, set] = useAtom(atom);
  return { value, set };
};
