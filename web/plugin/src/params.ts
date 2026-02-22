import { decode } from "@msgpack/msgpack";
import { InfoSchema, Info } from "./protocol/param_info";
import {
  useBooleanAtom,
  useExtended,
  useGrab,
  useNumericAtom,
  useStringAtom,
} from "./stores_react";

const infoExtended = (b: Uint8Array) => InfoSchema.parse(decode(b));
type TypedInfo<T, I> = {
  infoTransformer: (info: Info) => I;
  atomGetter: (path: string) => [T, (v: T) => void];
};

/**
 * @group Types
 */
export type Param<T, I> = {
  info: I;
  grab: () => void;
  release: () => void;
  value: T;
  set: (v: T) => void;
};

const makeUseParam =
  <T, I>(typedInfo: TypedInfo<T, I>) =>
  (param: string): Param<T, I> => {
    const info = typedInfo.infoTransformer(
      useExtended(`params-info/${param}`, infoExtended),
    );
    const { grab, release } = useGrab(`params-grabbed/${param}`);
    const [value, set] = typedInfo.atomGetter(`params/${param}`);
    return { info, grab, release, value, set };
  };

/**
 * @group Types
 */
export type NumericParamInfo = {
  title: string;
  default: number;
  valid_range: readonly [number, number];
  units: string;
};

/**
 * @group Hooks
 */
export const useNumericParam = makeUseParam<number, NumericParamInfo>({
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

/**
 * @group Types
 */
export type EnumParamInfo = {
  title: string;
  default: string;
  values: readonly string[];
};

/**
 * @group Hooks
 */
export const useEnumParam = makeUseParam<string, EnumParamInfo>({
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

/**
 * @group Types
 */
export type SwitchParamInfo = {
  title: string;
  default: boolean;
};

/**
 * @group Hooks
 */
export const useSwitchParam = makeUseParam<boolean, SwitchParamInfo>({
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
