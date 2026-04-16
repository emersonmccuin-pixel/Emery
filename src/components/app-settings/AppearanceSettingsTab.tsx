import { useAppStore } from "@/store";
import { themes } from "@/themes";

function AppearanceSettingsTab() {
  const activeThemeId = useAppStore((s) => s.activeThemeId);

  return (
    <article className="overview-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Appearance</p>
          <strong>Theme</strong>
        </div>
      </div>
      <div className="theme-picker-grid">
        {Object.entries(themes).map(([id, theme]) => {
          const isActive = id === activeThemeId;
          return (
            <button
              key={id}
              type="button"
              className={`theme-card${isActive ? " theme-card--active" : ""}`}
              style={
                isActive
                  ? {
                      borderColor: theme["--center-tint"],
                      boxShadow: `0 0 0 1px ${theme["--center-tint"]}, 0 0 16px color-mix(in srgb, ${theme["--center-tint"]} 40%, transparent)`,
                    }
                  : undefined
              }
              onClick={() => useAppStore.getState().setActiveThemeId(id)}
            >
              <div
                className="theme-card__preview"
                style={{ background: theme["--hud-bg"] }}
              >
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--rail-projects-tint"],
                  }}
                />
                <div
                  className="theme-card__preview-panel theme-card__preview-panel--center"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--center-tint"],
                  }}
                />
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--rail-sessions-tint"],
                  }}
                />
              </div>

              <div className="theme-card__footer">
                <span className="theme-card__label">{theme.label}</span>
                <div className="theme-card__swatches">
                  {(
                    [
                      "--rail-projects-tint",
                      "--center-tint",
                      "--rail-sessions-tint",
                      "--hud-amber",
                      "--hud-purple",
                    ] as const
                  ).map((key) => (
                    <span
                      key={key}
                      className="theme-card__swatch"
                      style={{ backgroundColor: theme[key] }}
                    />
                  ))}
                </div>
              </div>

              {isActive ? (
                <div
                  className="theme-card__check"
                  style={{ color: theme["--center-tint"] }}
                >
                  ✓
                </div>
              ) : null}
            </button>
          );
        })}
      </div>
    </article>
  );
}

export default AppearanceSettingsTab;
