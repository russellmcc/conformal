import msgpackTransport from "./msgpack_transport";
import storesWithTransport from "./stores";
import wryTransport from "./wry_transport";
import { Info } from "./protocol/param_info";
import mockStore from "./mock_store";
import { Context } from "./stores_react";
import { ReactNode } from "react";

const stores = wryTransport
  ? storesWithTransport(msgpackTransport(wryTransport))
  : undefined;

/**
 * @group Component Props
 */
export type ProviderProps = {
  /** Mock information about parameters. Setting this allows you to iterate on the UI without having to run the full plug-in. */
  mockInfos?: Map<string, Info>;
  /** @hidden */
  children: ReactNode;
};

/**
 * Context provider for connection with the plug-in.
 *
 * This must wrap any react subtree that uses the hooks from this package.
 *
 * @group Components
 */
export const Provider = ({ mockInfos, children }: ProviderProps) => (
  <Context.Provider
    value={stores ?? mockStore(mockInfos ?? new Map<string, Info>())}
  >
    {children}
  </Context.Provider>
);

export default Provider;
