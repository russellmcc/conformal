import Root from "./Root.tsx";
import * as Client from "react-dom/client";
import "./index.css";

const domElement = document.querySelector("#root");

if (!(domElement == null)) {
  Client.createRoot(domElement).render(<Root />);
}

export {};
