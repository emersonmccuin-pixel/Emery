import { useAppStore } from "@/store";
import { themes } from "@/themes";
import { uiFonts, terminalFonts, type FontOption } from "@/fonts";

const FONT_PREVIEW_TEXT = "The quick brown fox 0123";

function FontPicker({
  fonts,
  activeFontId,
  onSelect,
}: {
  fonts: FontOption[];
  activeFontId: string;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="font-picker-grid">
      {fonts.map((font) => {
        const isActive = font.id === activeFontId;
        return (
          <button
            key={font.id}
            type="button"
            className={`font-card${isActive ? " font-card--active" : ""}`}
            onClick={() => onSelect(font.id)}
          >
            <div className="font-card__label">
              {font.label}
              {font.systemOnly ? (
                <span className="font-card__badge ml-2">system</span>
              ) : null}
            </div>
            <div
              className="font-card__preview"
              style={{ fontFamily: font.stack }}
            >
              {FONT_PREVIEW_TEXT}
            </div>
            {isActive ? <div className="font-card__check">&#10003;</div> : null}
          </button>
        );
      })}
    </div>
  );
}

function AppearanceSettingsTab() {
  const activeThemeId = useAppStore((s) => s.activeThemeId);
  const uiFontId = useAppStore((s) => s.uiFontId);
  const terminalFontId = useAppStore((s) => s.terminalFontId);

  return (
    <div className="flex flex-col gap-6">
      {/* Theme picker */}
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
                    &#10003;
                  </div>
                ) : null}
              </button>
            );
          })}
        </div>
      </article>

      {/* UI Font picker */}
      <article className="overview-card">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Typography</p>
            <strong>UI Font</strong>
          </div>
        </div>
        <FontPicker
          fonts={uiFonts}
          activeFontId={uiFontId}
          onSelect={(id) => useAppStore.getState().setUiFontId(id)}
        />
      </article>

      {/* Terminal Font picker */}
      <article className="overview-card">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Typography</p>
            <strong>Terminal Font</strong>
          </div>
        </div>
        <FontPicker
          fonts={terminalFonts}
          activeFontId={terminalFontId}
          onSelect={(id) => useAppStore.getState().setTerminalFontId(id)}
        />
      </article>
    </div>
  );
}

export default AppearanceSettingsTab;
