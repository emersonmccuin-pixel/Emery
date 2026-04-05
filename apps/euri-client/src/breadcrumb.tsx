import { useMemo } from "react";
import { navStore, useNavLayer } from "./nav-store";
import type { NavigationLayer } from "./nav-store";
import { useAppStore } from "./store";

type Crumb = { label: string; layer: NavigationLayer };

function buildCrumbs(current: NavigationLayer): Crumb[] {
  const crumbs: Crumb[] = [{ label: "EURI", layer: { layer: "home" } }];
  if (current.layer === "project" || current.layer === "agent") {
    crumbs.push({ label: current.projectId, layer: { layer: "project", projectId: current.projectId } });
  }
  if (current.layer === "agent") {
    crumbs.push({ label: current.sessionId, layer: current });
  }
  return crumbs;
}

export function Breadcrumb() {
  const navLayer = useNavLayer();
  const bootstrap = useAppStore((s) => s.bootstrap);
  const sessions = useAppStore((s) => s.sessions);
  const workItemDetails = useAppStore((s) => s.workItemDetails);

  const crumbs = useMemo(() => buildCrumbs(navLayer), [navLayer]);

  function resolveLabel(crumb: Crumb): string {
    const layer = crumb.layer;
    if (layer.layer === "home") return "EURI";
    if (layer.layer === "project") {
      const project = bootstrap?.projects.find((p) => p.id === layer.projectId);
      return project?.name ?? crumb.label;
    }
    if (layer.layer === "agent") {
      const session = sessions.find((s) => s.id === layer.sessionId);
      if (session?.work_item_id) {
        const wi = workItemDetails[session.work_item_id];
        if (wi) return wi.callsign;
      }
      return session?.title ?? session?.current_mode ?? crumb.label;
    }
    return crumb.label;
  }

  return (
    <nav className="breadcrumb-bar">
      {crumbs.map((crumb, i) => (
        <span key={i} className="breadcrumb-segment">
          {i > 0 ? <span className="breadcrumb-sep">›</span> : null}
          {i < crumbs.length - 1 ? (
            <button
              className="breadcrumb-link"
              onClick={() => {
                const l = crumb.layer;
                if (l.layer === "home") navStore.goHome();
                else if (l.layer === "project") navStore.goToProject(l.projectId);
              }}
            >
              {resolveLabel(crumb)}
            </button>
          ) : (
            <span className="breadcrumb-current">{resolveLabel(crumb)}</span>
          )}
        </span>
      ))}
    </nav>
  );
}
