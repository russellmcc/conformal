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
 * Return value of parameter hooks.
 *
 * @typeParam T - The type of the parameter value.
 * @typeParam I - The type of information about the parameter.
 * @group Types
 */
export type Param<T, I> = {
  /** Information about the parameter */
  info: I;
  /** Grab the parameter */
  grab: () => void;
  /** Release (un-grab)the parameter */
  release: () => void;
  /** The current value of the parameter */
  value: T;
  /** Set the value of the parameter */
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
 * Information about a numeric parameter.
 * @group Types
 */
export type NumericParamInfo = {
  /** The title of the parameter */
  title: string;
  /** The default value of the parameter */
  default: number;
  /** The valid range of the parameter */
  valid_range: readonly [number, number];
  /** The units of the parameter */
  units: string;
};

/**
 * Hook to get a numeric parameter.
 *
 * {@includeCode ../examples/basic.tsx#hook-usage}
 *
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
 * Information about an enum parameter.
 *
 * @group Types
 */
export type EnumParamInfo = {
  /** The title of the parameter */
  title: string;
  /** The default value of the parameter */
  default: string;
  /** The values of the parameter */
  values: readonly string[];
};

/**
 * Hook to get an enum parameter.
 *
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
 * Information about a switch parameter.
 * @group Types
 */
export type SwitchParamInfo = {
  /** The title of the parameter */
  title: string;
  /** The default value of the parameter */
  default: boolean;
};

/**
 * Hook to get a switch parameter.
 *
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
