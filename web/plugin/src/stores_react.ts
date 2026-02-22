import { useAtom, useAtomValue } from "jotai";
import { createContext, useContext } from "react";
import { ExtendedInfo, Stores } from "./stores";

export const Context = createContext<Stores | null>(null);

/**
 *
 * @group Hooks
 * @category Advanced
 */
export const useStores = (): Stores => {
  const storage = useContext(Context);
  if (!storage) {
    throw new Error("No storage provider!");
  }
  return storage;
};

export const useGenericValue = (path: string) =>
  useAtomValue(useStores().generic(path));

export const useGenericAtom = (path: string) =>
  useAtom(useStores().generic(path));

export const useNumericValue = (path: string): number =>
  useAtomValue(useStores().numeric(path));

export const useNumericAtom = (path: string) =>
  useAtom(useStores().numeric(path));

export const useStringValue = (path: string): string =>
  useAtomValue(useStores().string(path));

export const useStringAtom = (path: string) =>
  useAtom(useStores().string(path));

export const useBooleanValue = (path: string): boolean =>
  useAtomValue(useStores().boolean(path));

export const useBooleanAtom = (path: string) =>
  useAtom(useStores().boolean(path));

export const useBytesValue = (path: string): Uint8Array =>
  useAtomValue(useStores().bytes(path));

export const useBytesAtom = (path: string) => useAtom(useStores().bytes(path));

export type Grab = {
  grab: () => void;
  release: () => void;
};

export const useGrab = (path: string): Grab => {
  const storage = useStores();
  const grab = useAtom(storage.grabbed(path))[1];
  return {
    grab: () => {
      grab(true);
    },
    release: () => {
      grab(false);
    },
  };
};

export const useExtended = <T>(path: string, info: ExtendedInfo<T>): T =>
  useAtomValue(useStores().extended([path, info]));
