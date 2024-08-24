import { Atom, WritableAtom, atom } from "jotai";
import { Response, Transport, Value } from "./protocol";
import { atomFamily } from "jotai/utils";

export type Family<T> = (
  path: string,
) => WritableAtom<Promise<T> | T, [T], void>;

// Note that this can throw on error.
export type ExtendedInfo<T> = (b: Uint8Array) => T;

export interface Stores {
  generic: Family<Value>;
  numeric: Family<number>;
  string: Family<string>;
  boolean: Family<boolean>;
  bytes: Family<Uint8Array>;
  grabbed: (path: string) => WritableAtom<null, [boolean], void>;
  extended: <T>(pathInfo: [string, ExtendedInfo<T>]) => Atom<Promise<T> | T>;
}

interface TypedInfo<T> {
  to: (x: Value) => T | undefined;
  from(x: T): Value;
}

const numberInfo: TypedInfo<number> = {
  to: (x) => {
    if (typeof x === "number") {
      return x;
    } else {
      return undefined;
    }
  },
  from: (x) => x,
};

const stringInfo: TypedInfo<string> = {
  to: (x) => {
    if (typeof x === "string") {
      return x;
    } else {
      return undefined;
    }
  },
  from: (x) => x,
};

const booleanInfo: TypedInfo<boolean> = {
  to: (x) => {
    if (typeof x === "boolean") {
      return x;
    } else {
      return undefined;
    }
  },
  from: (x) => x,
};

const bytesInfo: TypedInfo<Uint8Array> = {
  to: (x) => {
    if (x instanceof Uint8Array) {
      return x;
    } else {
      return undefined;
    }
  },
  from: (x) => x,
};

const derivedStore = <T>(
  info: TypedInfo<T>,
  generic: Family<Value>,
): Family<T> =>
  atomFamily((path: string) => {
    const base = generic(path);
    return atom(
      (get) => {
        const b = get(base);
        const matchOrThrow = (v: Value): T => {
          const x = info.to(v);
          if (x !== undefined) {
            return x;
          } else {
            throw new Error("Invalid type");
          }
        };
        if (b instanceof Promise) {
          return b.then(matchOrThrow);
        } else {
          return matchOrThrow(b);
        }
      },
      (_get, set, update) => {
        set(base, info.from(update));
      },
    );
  });

const grabbedStore = (
  transport: Transport,
): ((path: string) => WritableAtom<null, [boolean], void>) =>
  atomFamily((path: string) => {
    const baseNumeric = atom(0);
    return atom(null, (get, set, increment: boolean) => {
      set(baseNumeric, (old) => Math.max(0, old + (increment ? 1 : -1)));
      transport.request({
        m: "set",
        path,
        value: get(baseNumeric) > 0,
      });
    });
  });

const createGeneric = (transport: Transport): Family<Value> => {
  const setters = new Map<string, (v: Promise<Value> | Value) => void>();
  transport.setOnResponse((response: Response) => {
    if (response.m === "values") {
      for (const [path, value] of Object.entries(response.values)) {
        setters.get(path)?.(value);
      }
    } else if (response.m === "subscribe_error") {
      setters.get(response.path)?.(
        Promise.reject(new Error(`Subscribe Error at ${response.path}`)),
      );
    }
  });

  return atomFamily((path: string) => {
    // Due to jotai's suspension model, we have to load the value at this path at _init_
    // time rather than mount time. This is because `onMount` is called from `useEffect`,
    // which is called _after_ the suspense is finished - so if we set an infinitely pending state
    // during init, `onMount` will never be called and we'll never actually subscribe to the path.
    //
    // Note that this value won't be up-to-date so there may be a flash of stale data before
    // the subscription is established in `onMount`.
    const resolveReject = {
      contents: undefined as
        | undefined
        | [(value: Value) => void, (e: Error) => void],
    };
    const baseAtom = atom<Promise<Value> | Value>(
      new Promise<Value>((resolve, reject) => {
        resolveReject.contents = [resolve, reject];
      }),
    );
    setters.set(path, (value) => {
      setters.delete(path);
      transport.request({ m: "unsubscribe", path });
      const [resolve, reject] = resolveReject.contents!;
      if (value instanceof Promise) {
        value.then(resolve, reject);
      } else {
        resolve(value);
      }
    });
    // Send an initial subscribe to get the initial value.
    transport.request({ m: "subscribe", path });

    // Subscribe whenever the atom is mounted.
    baseAtom.onMount = (set) => {
      setters.set(path, (value) => {
        set(value);
      });
      transport.request({ m: "subscribe", path });
      return () => {
        setters.delete(path);
        transport.request({ m: "unsubscribe", path });
      };
    };

    // Return a read/write version of the atom
    return atom(
      (get) => get(baseAtom),
      (_get, set, update) => {
        set(baseAtom, update);
        transport.request({ m: "set", path, value: update });
      },
    );
  });
};

const extendedStore = (bytes: Family<Uint8Array>): Stores["extended"] =>
  atomFamily(
    <T>([path, info]: [string, ExtendedInfo<T>]) =>
      atom((get) => {
        const b = get(bytes(path));
        if (b instanceof Promise) {
          return b.then(info);
        } else {
          return info(b);
        }
      }),
    <T>(x: [string, T], y: [string, T]) => x[0] === y[0] && x[1] === y[1],
  );

export const storesFromGenericStore = (
  generic: Family<Value>,
  grabbed: (path: string) => WritableAtom<null, [boolean], void>,
): Stores => ({
  generic,
  numeric: derivedStore(numberInfo, generic),
  string: derivedStore(stringInfo, generic),
  boolean: derivedStore(booleanInfo, generic),
  bytes: derivedStore(bytesInfo, generic),
  grabbed,
  extended: extendedStore(derivedStore(bytesInfo, generic)),
});

export const storesWithTransport = (transport: Transport): Stores =>
  storesFromGenericStore(createGeneric(transport), grabbedStore(transport));

export default storesWithTransport;
