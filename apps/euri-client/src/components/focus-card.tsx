import { useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { ContextMenu, type ContextMenuItem } from "./context-menu";
import { navStore } from "../nav-store";
import { appStore } from "../store";
import type { ProjectSummary, SessionSummary } from "../types";

type FocusCardProps = {
  project: ProjectSummary;
  sessions: SessionSummary[];
};

function relativeTime(epochMs: number): string {
  const diffMs = Date.now() - epochMs;
  const diffSec = Math.floor(diffMs / 1000);
  if (diffSec < 60) return "just now";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ago`;
}

type CardContextMenu = {
  x: number;
  y: number;
  items: ContextMenuItem[];
} | null;

export function FocusCard({ project, sessions }: FocusCardProps) {
  const [contextMenu, setContextMenu] = useState<CardContextMenu>(null);

  const projectSessions = sessions.filter((s) => s.project_id === project.id);
  const activeSessions = projectSessions.filter((s) => s.runtime_state === "running");
  const needsAttention = projectSessions.filter((s) => s.activity_state === "needs_input");
  const liveSessions = projectSessions.filter((s) => s.live);

  const mostRecent = projectSessions.reduce<number | null>((latest, s) => {
    const ts = s.updated_at ?? s.started_at ?? s.created_at;
    if (ts && (latest === null || ts > latest)) return ts;
    return latest;
  }, null);

  const subtitle = project.slug;

  function handleClick() {
    navStore.goToProject(project.id);
  }

  function handleContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: "Open",
          onClick: () => navStore.goToProject(project.id),
        },
        {
          label: "Unpin from focus",
          onClick: () => appStore.unpinProject(project.id),
        },
      ],
    });
  }

  return (
    <>
      <Card
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        style={{
          cursor: "pointer",
          position: "relative",
          minWidth: 240,
          maxWidth: 320,
          flex: "1 1 280px",
          transition: "border-color 0.2s, box-shadow 0.2s, transform 0.15s",
        }}
        className="focus-card-hoverable"
      >
        {/* Attention indicator */}
        {needsAttention.length > 0 && (
          <span
            style={{
              position: "absolute",
              top: 14,
              right: 14,
              width: 10,
              height: 10,
              borderRadius: "50%",
              backgroundColor: "var(--accent, #d4a03c)",
              animation: "focus-card-blink 1.2s ease-in-out infinite",
            }}
          />
        )}

        <CardContent
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 12,
            padding: 20,
          }}
        >
          {/* Project name */}
          <div>
            <div
              style={{
                fontSize: 18,
                fontWeight: 700,
                color: "var(--text-primary)",
                letterSpacing: "0.04em",
                lineHeight: 1.3,
              }}
            >
              {project.name}
            </div>
            <div
              style={{
                fontSize: 12,
                color: "var(--text-secondary, #8a8a9a)",
                letterSpacing: "0.08em",
                marginTop: 2,
              }}
            >
              {subtitle}
            </div>
          </div>

          {/* Stats grid */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "8px 16px",
              fontSize: 12,
              color: "var(--text-secondary, #8a8a9a)",
            }}
          >
            <div>
              <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>
                {activeSessions.length}
              </span>{" "}
              running
            </div>
            <div>
              <span
                style={{
                  color: needsAttention.length > 0 ? "var(--accent, #d4a03c)" : "var(--text-primary)",
                  fontWeight: 600,
                }}
              >
                {needsAttention.length}
              </span>{" "}
              needs input
            </div>
            <div>
              <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>
                {liveSessions.length}
              </span>{" "}
              live
            </div>
            <div>
              {mostRecent ? relativeTime(mostRecent) : "no activity"}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Inline style for blink animation and hover */}
      <style>{`
        @keyframes focus-card-blink {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.3; }
        }
        .focus-card-hoverable:hover {
          border-color: var(--accent, #d4a03c) !important;
          box-shadow: 0 0 0 1px rgba(42,42,58,0.6), 0 0 30px rgba(212, 160, 60, 0.12) !important;
          transform: translateY(-1px);
        }
      `}</style>

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenu.items}
          onClose={() => setContextMenu(null)}
        />
      )}
    </>
  );
}
