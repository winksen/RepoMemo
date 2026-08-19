import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/fjalla-one/latin-400.css";
import "@fontsource-variable/recursive";
import { App } from "./App";
import "./styles.css";
import "./blueprint-refinement.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
