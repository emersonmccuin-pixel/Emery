import { SupervisorDot } from "./components/supervisor-dot";

export function Topbar() {
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-inner" data-tauri-drag-region>
        <span className="topbar-brand" data-tauri-drag-region>EURI</span>
        <SupervisorDot />
      </div>
    </header>
  );
}
