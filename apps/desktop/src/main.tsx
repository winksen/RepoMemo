import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/fjalla-one/latin-400.css";
import "@fontsource-variable/recursive";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-500.css";
import "@fontsource/jetbrains-mono/latin-600.css";
import { App } from "./App";
import "./styles.css";
import "./blueprint-refinement.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
