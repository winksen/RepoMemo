import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/fjalla-one/latin-400.css";
import "@fontsource-variable/plus-jakarta-sans";
import { App } from "./App";
import { ToastViewport } from "./components/ui/toast";
import "./styles.css";
import "./blueprint-refinement.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <ToastViewport />
  </React.StrictMode>,
);
