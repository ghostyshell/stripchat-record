import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Overlay title bar inset: the stylesheet keys off this, and the browser
// preview must pass ?os=mac to see the macOS spacing.
const isTauri = "__TAURI_INTERNALS__" in window;
if (isTauri || new URLSearchParams(location.search).get("os") === "mac") {
  document.documentElement.dataset.os = "mac";
}

async function boot() {
  // ?mock=1 installs a fixture bridge before React mounts, so the first
  // snapshot call already has an invoke().
  if (new URLSearchParams(location.search).has("mock")) {
    await import("./mock");
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}

void boot();
