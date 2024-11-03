// Temporary workaround for https://github.com/oven-sh/bun/issues/4890
/// <reference lib="dom" />
/// <reference lib="dom.iterable" />

import App from "./App.tsx";
import * as Jotai from "jotai";
import { StrictMode, Suspense } from "react";
import * as Client from "react-dom/client";
import { Provider } from "@conformal/plugin";
import "./index.css";
import infos from "./mock_infos.ts";

const domElement = document.querySelector("#root");

if (!(domElement == null)) {
  Client.createRoot(domElement).render(
    <StrictMode>
      <Jotai.Provider>
        <Provider mockInfos={infos}>
          <Suspense fallback={<></>}>
            <App />
          </Suspense>
        </Provider>
      </Jotai.Provider>
    </StrictMode>,
  );
}

export {};
