import { atomFamily } from "jotai/utils";
import { atom } from "jotai";
import { encode } from "@msgpack/msgpack";
import { Info } from "./protocol/param_info";
import { Family, storesFromGenericStore } from "./stores";
import { Value } from "./protocol";

const mockGeneric = (infos: Map<string, Info>): Family<Value> =>
  atomFamily((path) => {
    const paramPath = path.match(/^params\/(.*)$/);
    if (paramPath) {
      const param = paramPath[1]!;
      const info = infos.get(param);
      if (!info) {
        throw new Error(`Unknown param: ${param}`);
      }
      if (info.type_specific.t === "switch") {
        return atom<Value>({ bool: info.type_specific.default });
      } else if (info.type_specific.t === "numeric") {
        return atom<Value>({ numeric: info.type_specific.default });
      } else {
        return atom<Value>({ string: info.type_specific.default });
      }
    }

    const prefsPath = path.match(/^prefs\/(.*)$/);
    if (prefsPath) {
      // All prefs are "false" in mock stores
      return atom<Value>({ bool: false });
    }

    // Using a regex, check if path is like `params-info/${param}`
    // and if so, return the info encoded as bytes.
    // also it's very important that my regex captures "param".
    const paramInfoPath = path.match(/^params-info\/(.*)$/);
    if (paramInfoPath) {
      const param = paramInfoPath[1]!;
      const info = infos.get(param);
      if (!info) {
        throw new Error(`Unknown param: ${param}`);
      }

      return atom<Value>({ bytes: encode(info) });
    }

    if (path === "ui-state") {
      return atom<Value>({ bytes: new Uint8Array() });
    }

    throw new Error(`Unknown path: ${path}`);
  });

const mockGrabbed = () =>
  atomFamily((path: string) => {
    const grabbedPath = path.match(/^params-grabbed\/(.*)$/);
    if (!grabbedPath) {
      throw new Error(`Unknown path: ${path}`);
    }
    const baseNumeric = atom(0);

    return atom(null, (_, set, increment: boolean) => {
      set(baseNumeric, (old) => Math.max(0, old + (increment ? 1 : -1)));
    });
  });

const mockStore = (infos: Map<string, Info>) =>
  storesFromGenericStore(mockGeneric(infos), mockGrabbed());
export default mockStore;
