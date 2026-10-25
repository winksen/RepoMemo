import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/fjalla-one/latin-400.css";
import "@fontsource-variable/plus-jakarta-sans";
import { App } from "./App";
import { ToastViewport } from "./components/ui/toast";
import { applyRuntimeConfig, loadRuntimeConfig } from "./lib/runtimeConfig";
import "./styles";

// A browser asks the host for its configuration first (V0.4.14); the desktop window gets its server from its shell.
async function start() {
  if (!("__TAURI_INTERNALS__" in window)) applyRuntimeConfig(await loadRuntimeConfig());
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
      <ToastViewport />
    </React.StrictMode>,
  );
}

void start();
