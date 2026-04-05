import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Apply persisted theme before first render to avoid flash
const savedTheme = localStorage.getItem("euri.theme") ?? "vaporwave";
document.documentElement.dataset.theme = savedTheme;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
