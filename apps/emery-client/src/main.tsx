import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { loadOverrides, applyOverrides } from "./appearance";

// Apply persisted theme before first render to avoid flash.
// Upgrade legacy theme installs to the new "default" (mission-control) theme.
const LEGACY_THEMES = new Set(["vaporwave", "cyberpunk", "dark", "neutral-dark", "fallout", "vapor", "synthwave", "deep-ocean", "amber", "aurora", "noir", "mars", "mission-control"]);
const savedTheme =
  localStorage.getItem("emery.theme") ??
  localStorage.getItem("euri.theme") ??
  "default";
const resolvedTheme = LEGACY_THEMES.has(savedTheme) ? "default" : savedTheme;
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
