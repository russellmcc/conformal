import msgpackTransport from "./msgpack_transport";
import storesWithTransport from "./stores";
import wryTransport from "./wry_transport";
import { Info } from "./protocol/param_info";
import mockStore from "./mock_store";
import { Context } from "./stores_react";

const stores = wryTransport
  ? storesWithTransport(msgpackTransport(wryTransport))
  : undefined;

export const Provider = ({
  mockInfos,
  children,
}: {
  mockInfos: Map<string, Info>;
  children: React.ReactNode;
}) => (
  <Context.Provider value={stores ?? mockStore(mockInfos)}>
    {children}
  </Context.Provider>
);

export default Provider;
