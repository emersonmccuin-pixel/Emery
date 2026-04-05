import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Apply persisted theme before first render to avoid flash.
// Upgrade legacy "vaporwave" installs to the new cyberpunk default.
const savedTheme = localStorage.getItem("euri.theme") ?? "cyberpunk";
const resolvedTheme = savedTheme === "vaporwave" ? "cyberpunk" : savedTheme;
document.documentElement.dataset.theme = resolvedTheme;

if (savedTheme !== resolvedTheme) {
  localStorage.setItem("euri.theme", resolvedTheme);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
