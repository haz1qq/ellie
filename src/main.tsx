import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import MiniBar from "./components/MiniBar";
import { desktop } from "./lib/desktop";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Missing application root");
const windowLabel = desktop.available() ? getCurrentWindow().label : "main";
document.documentElement.dataset.window = windowLabel;
document.body.dataset.window = windowLabel;
ReactDOM.createRoot(root).render(
  <React.StrictMode>
    {windowLabel === "mini" ? <MiniBar /> : <App />}
  </React.StrictMode>,
);
