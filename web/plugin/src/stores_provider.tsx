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
  mockInfos?: Map<string, Info>;
  children: ReactNode;
};

export const Provider = ({ mockInfos, children }: ProviderProps) => (
  <Context.Provider
    value={stores ?? mockStore(mockInfos ?? new Map<string, Info>())}
  >
    {children}
  </Context.Provider>
);

export default Provider;
