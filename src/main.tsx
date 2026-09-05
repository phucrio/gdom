import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App.tsx";
import { createTauriBackend } from "./ipc/tauri.ts";

const rootElement = document.getElementById("root");

if (rootElement === null) {
  throw new TypeError("Missing root element");
}

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App backend={createTauriBackend()} />
  </React.StrictMode>,
);
