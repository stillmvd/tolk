import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./app.css";
import { Card } from "./Card";

const label = getCurrentWindow().label.split("-")[0];
document.documentElement.dataset.window = label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{label === "card" ? <Card /> : <main />}</React.StrictMode>,
);
