import { SupervisorDot } from "./components/supervisor-dot";
import { navStore, useNavLayer } from "./nav-store";
import { useAppStore } from "./store";

function BackButton() {
  const navLayer = useNavLayer();
  const projects = useAppStore((s) => s.bootstrap?.projects ?? []);

  if (navLayer.layer === "home") return null;

  let label = "\u2190 Back";
  if (navLayer.layer === "agent") {
    const project = projects.find((p) => p.id === navLayer.projectId);
    if (project) {
      label = `\u2190 ${project.name}`;
    }
  }

  return (
    <button
      className="topbar-back-btn"
      onClick={() => navStore.goBack()}
      title="Go back"
    >
      {label}
    </button>
  );
}

export function Topbar() {
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-inner" data-tauri-drag-region>
        <span className="topbar-brand" data-tauri-drag-region>Emery</span>
        <SupervisorDot />
        <BackButton />
      </div>
    </header>
  );
}
