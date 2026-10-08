import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { Popover } from "./screens/Popover";
import { installZoomShortcuts } from "./zoom";
import { ZoomIndicator } from "./components/ZoomIndicator";
import { loadLang } from "./i18n";
import "./index.css";

// One bundle, two windows: the menubar popover is the same app under another label.
const isPopover = getCurrentWindow().label === "popover";
if (isPopover) document.documentElement.classList.add("popover");
const removeZoomShortcuts = installZoomShortcuts();
if (import.meta.hot) import.meta.hot.dispose(removeZoomShortcuts);

// The language is read before the first paint so neither window flashes the wrong one.
async function start() {
  await loadLang();
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      {isPopover ? <Popover /> : <App />}
      <ZoomIndicator />
    </React.StrictMode>,
  );
}
void start();
