import { useMemo, useState } from "react";
import { useAppStore } from "../store";
import { navStore } from "../nav-store";
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
    const query = filter.trim().toLowerCase();
    if (!query) return focusProjects;
    return focusProjects.filter((p) => p.name.toLowerCase().includes(query));
  }, [focusProjects, filter]);

  return (
    <div className="home-view">
      <div className="content-frame-wide">
        <header className="mb-8">
          <h1 className="text-2xl font-bold text-[var(--text-primary)] mb-2">Dashboard</h1>
          <p className="text-[var(--text-secondary)]">Manage your active projects and agents.</p>
        </header>

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

        {focusProjects.length > 0 ? (
          <section>
            <div className="flex items-center justify-between mb-4">
              <h2 className="all-projects-header m-0">
                {filter ? "Search Results" : "Pinned Projects"}
              </h2>
            </div>

            {visibleProjects.length > 0 ? (
              <div className="focus-card-grid">
                {visibleProjects.map((project) => (
                  <FocusCard
                    key={project.id}
                    project={project}
                    sessions={sessions}
                  />
                ))}
              </div>
            ) : (
              <div className="empty-pane bg-[var(--surface-sunken)] rounded-lg p-10 border border-[var(--border-subtle)]">
                <p>No projects found matching "{filter}"</p>
              </div>
            )}
          </section>
        ) : (
          <div className="empty-pane bg-[var(--surface-sunken)] rounded-lg p-20 border border-[var(--border-subtle)] flex flex-col items-center justify-center text-center">
            <p className="text-lg text-[var(--text-primary)] mb-4 font-semibold">Welcome to EURI</p>
            <p className="text-[var(--text-secondary)] mb-8 max-w-md">
              Pin a project from the sidebar to get started, or create a new project to begin your workflow.
            </p>
            <button
              className="btn-sm bg-[var(--accent)] text-[var(--text-on-primary)] hover:brightness-110 font-bold px-6 py-2"
              onClick={() => navStore.openModal({ modal: "create_project" })}
            >
              + Create First Project
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
