import { decode } from "@msgpack/msgpack";
import { Info } from "./protocol/param_info";
import {
  useBooleanAtom,
  useExtended,
  useGrab,
  useNumericAtom,
  useStringAtom,
} from "./stores_react";

const infoExtended = (b: Uint8Array) => Info.parse(decode(b));
type TypedInfo<T, I> = {
  infoTransformer: (info: Info) => I;
  atomGetter: (path: string) => [T, (v: T) => void];
};

const makeUseParam =
  <T, I>(typedInfo: TypedInfo<T, I>) =>
  (param: string) => {
    const info = typedInfo.infoTransformer(
      useExtended(`params-info/${param}`, infoExtended),
    );
    const { grab, release } = useGrab(`params-grabbed/${param}`);
    const [value, set] = typedInfo.atomGetter(`params/${param}`);
    return { info, grab, release, value, set };
  };

export const useNumericParam = makeUseParam({
  infoTransformer: (info) => {
    if (info.type_specific.t === "numeric") {
      return {
        title: info.title,
        default: info.type_specific.default,
        valid_range: info.type_specific.valid_range,
        units: info.type_specific.units,
      };
    } else {
      throw new Error("Wrong info type.");
    }
  },
  atomGetter: useNumericAtom,
});

export const useEnumParam = makeUseParam({
  infoTransformer: (info) => {
    if (info.type_specific.t === "enum") {
      return {
        title: info.title,
        default: info.type_specific.default,
        values: info.type_specific.values,
      };
    } else {
      throw new Error("Wrong info type.");
    }
  },
  atomGetter: useStringAtom,
});

export const useSwitchParam = makeUseParam({
  infoTransformer: (info) => {
    if (info.type_specific.t === "switch") {
      return {
        title: info.title,
        default: info.type_specific.default,
      };
    } else {
      throw new Error("Wrong info type.");
    }
  },
  atomGetter: useBooleanAtom,
});
