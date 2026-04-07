import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { loadOverrides, applyOverrides } from "./appearance";
import { getStoredValue, setStoredValue } from "./utils/local-storage";

// Apply persisted theme before first render to avoid flash.
// Upgrade legacy theme installs to the new "default" (mission-control) theme.
const LEGACY_THEMES = new Set(["vaporwave", "cyberpunk", "dark", "neutral-dark", "fallout", "vapor", "synthwave", "deep-ocean", "amber", "aurora", "noir", "mars", "mission-control"]);
const savedTheme =
  getStoredValue("emery.theme", "euri.theme") ??
  "default";
const resolvedTheme = LEGACY_THEMES.has(savedTheme) ? "default" : savedTheme;
document.documentElement.dataset.theme = resolvedTheme;

if (savedTheme !== resolvedTheme || localStorage.getItem("emery.theme") !== resolvedTheme) {
  setStoredValue("emery.theme", resolvedTheme, "euri.theme");
}

// Apply persisted appearance overrides (brightness, font scale, etc.)
applyOverrides(loadOverrides());

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
