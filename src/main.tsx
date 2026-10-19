import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./app.css";
import { Card } from "./Card";
import { Settings } from "./Settings";
import { Translator } from "./Translator";

const label = getCurrentWindow().label.split("-")[0];
document.documentElement.dataset.window = label;

const VIEWS: Record<string, React.ReactNode> = { card: <Card />, translator: <Translator />, settings: <Settings /> };

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode>{VIEWS[label] ?? <main />}</React.StrictMode>);
