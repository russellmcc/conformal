/**
 * This package is part of the Conformal Framework! For high-level documentation and tutorials,
 * please see [the main documentation website](https://russellmcc.github.io/conformal)!
 *
 * This package contains functionality to connect your React UIs to the parameters of a Conformal plug-in.
 *
 * ## Basic usage
 *
 * {@includeCode ../examples/basic.tsx#hook-usage}
 *
 * The main entry points to this package are the {@link useNumericParam}, {@link useEnumParam} and
 * {@link useSwitchParam} hooks, which give quick access to the parameters of your plug-in. In
 * addition to reading parameters, you can also set them using the same hook:
 *
 * {@includeCode ../examples/basic.tsx#write-hook-usage}
 *
 * ## Required providers
 *
 * In order to use the hooks, your application must be wrapped in a {@link Provider} component,
 * which itself requires a `Jotai.Provider` from [jotai](https://jotai.org) to be present.
 *
 * {@includeCode ../examples/basic.tsx#required-providers}
 *
 * ## Mocking parameters
 *
 * It can be convenient to mock the parameters of your plug-in so you can iterate on the UI
 * without having to run the full plug-in. Providing a {@link ProviderProps.mockInfos | mockInfos} prop to the {@link Provider} component
 * make hooks fall back to using mock values of parameters when there is no plug-in present.
 *
 * {@includeCode ../examples/basic.tsx#mocking-parameters}
 *
 * @groupDescription Components
 * React components exported by this package.
 *
 * @groupDescription Hooks
 * React hooks exported by this package.
 *
 * @module
 *
 */

export { Info } from "./protocol/param_info";
export { storesFromGenericStore } from "./stores";
export type { Family, Stores } from "./stores";
export {
  useStores,
  useBooleanAtom,
  useBooleanValue,
  useBytesAtom,
  useBytesValue,
  useExtended,
  useGenericAtom,
  useGenericValue,
  useGrab,
  useNumericAtom,
  useNumericValue,
  useStringAtom,
  useStringValue,
} from "./stores_react";
export { default as Provider, type ProviderProps } from "./stores_provider";
export { useEnumParam, useNumericParam, useSwitchParam } from "./params";
export { default as DevModeTools } from "./DevModeTools";
export { useUiStateAtom, codecFromZod } from "./ui_state";
export type { Codec } from "./ui_state";
export { UiStateProvider } from "./ui_state_provider";
