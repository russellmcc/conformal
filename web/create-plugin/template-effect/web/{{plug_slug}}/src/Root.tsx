import { StrictMode, Suspense } from "react";
import { Provider } from "@conformal/plugin";
import infos from "./mock_infos.ts";
import App from "./App.tsx";

export const RootProviders = ({ children }: { children: React.ReactNode }) => (
  <StrictMode>
    <Provider mockInfos={infos}>
      <Suspense fallback={<></>}>{children}</Suspense>
    </Provider>
  </StrictMode>
);

export const Root = () => (
  <RootProviders>
    <App />
  </RootProviders>
);

export default Root;
