import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { loadOverrides, applyOverrides } from "./appearance";

// Apply persisted theme before first render to avoid flash.
// Upgrade legacy "vaporwave" installs to the new cyberpunk default.
const savedTheme =
  localStorage.getItem("emery.theme") ??
  localStorage.getItem("euri.theme") ??
  "cyberpunk";
const resolvedTheme = savedTheme === "vaporwave" ? "cyberpunk" : savedTheme;
document.documentElement.dataset.theme = resolvedTheme;

if (savedTheme !== resolvedTheme || localStorage.getItem("emery.theme") !== resolvedTheme) {
  localStorage.setItem("emery.theme", resolvedTheme);
}

// Apply persisted appearance overrides (brightness, font scale, etc.)
applyOverrides(loadOverrides());

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
