import { useMemo, useState } from "react";
import { useAppStore } from "../store";
import { FocusCard } from "../components/focus-card";
import type { ProjectSummary } from "../types";

export function HomeView() {
  const allProjects = useAppStore((s) => s.bootstrap?.projects ?? []);
  const sessions = useAppStore((s) => s.sessions);
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const [filter, setFilter] = useState("");

  const focusProjects = useMemo(() => {
    const result: ProjectSummary[] = [];
    for (const id of focusProjectIds) {
      const p = allProjects.find((proj) => proj.id === id && proj.archived_at === null);
      if (p) result.push(p);
    }
    return result;
  }, [allProjects, focusProjectIds]);

  const visibleProjects = useMemo(() => {
    if (!filter.trim()) return focusProjects;
    const query = filter.trim().toLowerCase();
    return focusProjects.filter((p) => p.name.toLowerCase().includes(query));
  }, [focusProjects, filter]);

  const showSearch = focusProjects.length > 0;

  return (
    <div className="content-frame">
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: focusProjects.length === 0 ? "center" : "flex-start",
          minHeight: "100%",
          padding: "64px 32px 32px",
          boxSizing: "border-box",
        }}
      >
        {showSearch && (
          <div className="home-search-wrap">
            <input
              className="home-search-input"
              type="text"
              placeholder="Filter projects…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              aria-label="Filter projects"
            />
            {filter && (
              <button
                className="home-search-clear"
                onClick={() => setFilter("")}
                aria-label="Clear filter"
                title="Clear"
              >
                ×
              </button>
            )}
          </div>
        )}

        {focusProjects.length > 0 ? (
          visibleProjects.length > 0 ? (
            <div
              style={{
                display: "flex",
                flexWrap: "wrap",
                gap: 20,
                justifyContent: "center",
                maxWidth: 1040,
                width: "100%",
              }}
            >
              {visibleProjects.map((project) => (
                <FocusCard
                  key={project.id}
                  project={project}
                  sessions={sessions}
                />
              ))}
            </div>
          ) : (
            <p
              style={{
                color: "var(--text-muted)",
                fontSize: "var(--text-sm)",
                letterSpacing: "0.04em",
              }}
            >
              No projects match "{filter}"
            </p>
          )
        ) : (
          <p
            style={{
              color: "var(--text-secondary, #8a8a9a)",
              fontSize: 14,
              letterSpacing: "0.04em",
            }}
          >
            Pin a project from the sidebar to get started
          </p>
        )}
      </div>
    </div>
  );
}
