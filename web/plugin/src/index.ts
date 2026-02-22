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
 * In order to use the hooks, your application must be wrapped in a {@link Provider} component.
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
 * ## UI State
 *
 * Conformal UI can store custom state unrelated to parameters that will persist
 * in the DAW save file. This allows you to retain UI state in the file.
 * The entry point for this is the {@link useUiState} hook, please
 * see the docs there.
 *
 * ## DevModeTools
 *
 * The {@link DevModeTools} component displays a toggle to switch between the
 * version of the UI that is embedded into the plug-in and a dev server for
 * iterative development.
 *
 * @groupDescription Components
 * React components exported by this package.
 *
 * @groupDescription Component Props
 * Props for React {@link Components}
 *
 * @groupDescription Hooks
 * React hooks exported by this package.
 *
 * @module
 *
 */

export type { Info } from "./protocol/param_info";
export { default as Provider, type ProviderProps } from "./stores_provider";
export {
  useEnumParam,
  type EnumParamInfo,
  useNumericParam,
  type NumericParamInfo,
  useSwitchParam,
  type SwitchParamInfo,
  type Param,
} from "./params";
export { default as DevModeTools } from "./DevModeTools";
export { useUiState, codecFromZod } from "./ui_state";
export type { Codec } from "./ui_state";
export {
  UiStateProvider,
  type UiStateProviderProps,
} from "./ui_state_provider";
