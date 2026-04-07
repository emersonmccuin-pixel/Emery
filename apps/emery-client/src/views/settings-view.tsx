import { useEffect, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import type { AccountSummary, McpServerSummary, VaultEntry, VaultLockStatus } from "../types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { pickFolder, listMcpServers, createMcpServer, updateMcpServer, deleteMcpServer, vaultStatus, vaultList, vaultGet, vaultSet, vaultDelete, vaultUnlock } from "../lib";
import {
  type AppearanceOverrides,
  loadOverrides,
  saveOverrides,
  applyOverrides,
  resetOverrides,
} from "../appearance";
import { setStoredValue } from "../utils/local-storage";

type SettingsTab = "accounts" | "appearance" | "agent-defaults" | "mcp-servers" | "github" | "knowledge" | "resolution" | "api-keys";

export function SettingsView() {
  const [activeTab, setActiveTab] = useState<SettingsTab>("accounts");

  const tabs: Array<{ id: SettingsTab; label: string }> = [
    { id: "accounts", label: "Accounts" },
    { id: "appearance", label: "Appearance" },
    { id: "agent-defaults", label: "Agent Defaults" },
    { id: "mcp-servers", label: "MCP Servers" },
    { id: "github", label: "GitHub" },
    { id: "knowledge", label: "Knowledge Store" },
    { id: "resolution", label: "Config Resolution" },
    { id: "api-keys", label: "API Keys" },
  ];

  return (
    <div className="modal-view-settings">
      <div className="global-settings-view">
        <div className="global-settings-header">
          <h2 className="global-settings-title">Settings</h2>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navStore.closeModal()}
          >
            Close
          </Button>
        </div>
        <div className="global-settings-body">
          <nav className="global-settings-sidebar">
          {tabs.map((tab) => (
            <Button
              key={tab.id}
              variant={activeTab === tab.id ? "default" : "ghost"}
              className={`global-settings-nav-item${activeTab === tab.id ? " active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </Button>
          ))}
          </nav>
          <div className="global-settings-content">
            {activeTab === "accounts" && <AccountsSection />}
            {activeTab === "appearance" && <AppearanceSection />}
            {activeTab === "agent-defaults" && <AgentDefaultsSection />}
            {activeTab === "mcp-servers" && <McpServersSection />}
            {activeTab === "github" && <GitHubSection />}
            {activeTab === "knowledge" && <KnowledgeStoreSection />}
            {activeTab === "resolution" && <ConfigResolutionSection />}
            {activeTab === "api-keys" && <ApiKeysSection />}
          </div>
        </div>
      </div>
    </div>
  );
}

// --- Accounts Section ---

function AccountsSection() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const [showAddForm, setShowAddForm] = useState(false);
  const [newLabel, setNewLabel] = useState("");
  const [newBinaryPath, setNewBinaryPath] = useState("");
  const [newConfigRoot, setNewConfigRoot] = useState("");
  const [newAgentKind, setNewAgentKind] = useState("claude");
  const [newSafetyMode, setNewSafetyMode] = useState("");

  const allAccounts: AccountSummary[] = bootstrap?.accounts ?? [];
  const accounts = allAccounts.filter((a) => a.status !== "disabled");
  const creating = loadingKeys["create-account"] ?? false;

  async function handleCreate() {
    if (!newLabel.trim()) return;
    await appStore.handleCreateAccount({
      label: newLabel.trim(),
      agent_kind: newAgentKind,
      binary_path: newBinaryPath.trim() || null,
      config_root: newConfigRoot.trim() || null,
      default_safety_mode: newSafetyMode || null,
    });
    setNewLabel("");
    setNewBinaryPath("");
    setNewConfigRoot("");
    setNewAgentKind("claude");
    setNewSafetyMode("");
    setShowAddForm(false);
  }

  async function handlePickConfigRoot() {
    const path = await pickFolder();
    if (path) setNewConfigRoot(path);
  }

  return (
    <Card className="settings-panel">
      <CardHeader>
        <div className="settings-panel-header-row">
          <div>
            <CardTitle className="settings-section-title">Accounts</CardTitle>
            <CardDescription>Manage the agent accounts available for launching sessions.</CardDescription>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="section-add-btn size-9"
            onClick={() => setShowAddForm((v) => !v)}
            title="Add account"
          >
            +
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">

      {showAddForm && (
        <div className="settings-add-form">
          <div className="settings-field-group">
            <label className="settings-label">Label</label>
            <Input
              className="settings-input"
              type="text"
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
                if (e.key === "Escape") setShowAddForm(false);
              }}
              placeholder="My Claude account"
              autoFocus
            />
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Agent kind</label>
            <select
              className="settings-input"
              value={newAgentKind}
              onChange={(e) => setNewAgentKind(e.target.value)}
            >
              <option value="claude">claude</option>
              <option value="codex">codex</option>
              <option value="gemini">gemini</option>
            </select>
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Safety mode (optional)</label>
            <select
              className="settings-input"
              value={newSafetyMode}
              onChange={(e) => setNewSafetyMode(e.target.value)}
            >
              <option value="">Default</option>
              <option value="cautious">cautious</option>
              <option value="autonomous">autonomous</option>
              <option value="yolo">yolo</option>
            </select>
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Agent config folder (optional)</label>
            <div className="settings-input-row">
              <Input
                className="settings-input"
                type="text"
                value={newConfigRoot}
                onChange={(e) => setNewConfigRoot(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleCreate();
                  if (e.key === "Escape") setShowAddForm(false);
                }}
                placeholder="~/.claude"
                style={{ flex: 1 }}
              />
              <Button variant="ghost" size="sm" onClick={() => void handlePickConfigRoot()}>
                Browse
              </Button>
            </div>
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Binary path (optional)</label>
            <Input
              className="settings-input"
              type="text"
              value={newBinaryPath}
              onChange={(e) => setNewBinaryPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
                if (e.key === "Escape") setShowAddForm(false);
              }}
              placeholder="/usr/local/bin/claude"
            />
          </div>
          <div className="settings-form-actions">
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleCreate()}
              disabled={creating || !newLabel.trim()}
            >
              {creating ? "Creating..." : "Create"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowAddForm(false)}
              disabled={creating}
            >
              Cancel
            </Button>
          </div>
        </div>
      )}

      {accounts.length === 0 ? (
        <div className="settings-empty-note">No accounts configured.</div>
      ) : (
        <div className="settings-account-list">
          {accounts.map((account) => (
            <AccountRow key={account.id} account={account} loadingKeys={loadingKeys} />
          ))}
        </div>
      )}
      </CardContent>
    </Card>
  );
}

function AccountRow({
  account,
  loadingKeys,
}: {
  account: AccountSummary;
  loadingKeys: Record<string, boolean>;
}) {
  const [editing, setEditing] = useState(false);
  const [labelInput, setLabelInput] = useState(account.label);
  const [binaryPathInput, setBinaryPathInput] = useState(account.binary_path ?? "");
  const [configRootInput, setConfigRootInput] = useState(account.config_root ?? "");
  const [safetyModeInput, setSafetyModeInput] = useState(account.default_safety_mode ?? "");
  const [confirmDelete, setConfirmDelete] = useState(false);

  const saving = loadingKeys[`update-account:${account.id}`] ?? false;
  const deleting = loadingKeys[`delete-account:${account.id}`] ?? false;

  async function handleSave() {
    if (!labelInput.trim()) return;
    await appStore.handleUpdateAccount(account.id, {
      label: labelInput.trim(),
      binary_path: binaryPathInput.trim() || null,
      config_root: configRootInput.trim() || null,
      default_safety_mode: safetyModeInput || null,
    });
    setEditing(false);
  }

  function handleCancelEdit() {
    setLabelInput(account.label);
    setBinaryPathInput(account.binary_path ?? "");
    setConfigRootInput(account.config_root ?? "");
    setSafetyModeInput(account.default_safety_mode ?? "");
    setEditing(false);
  }

  async function handlePickConfigRoot() {
    const path = await pickFolder();
    if (path) setConfigRootInput(path);
  }

  async function handleSetDefault() {
    if (account.is_default) return;
    await appStore.handleUpdateAccount(account.id, { is_default: true });
  }

  async function handleDelete() {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setConfirmDelete(false);
    await appStore.handleDeleteAccount(account.id);
  }

  return (
    <Card className="settings-account-row p-4">
      <div className="settings-account-info">
        {editing ? (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <div className="settings-field-group">
              <label className="settings-label">Label</label>
              <Input
                className="settings-input"
                type="text"
                value={labelInput}
                onChange={(e) => setLabelInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleSave();
                  if (e.key === "Escape") handleCancelEdit();
                }}
                autoFocus
              />
            </div>
            <div className="settings-field-group">
              <label className="settings-label">Agent config folder</label>
              <div className="settings-input-row">
                <Input
                  className="settings-input"
                  type="text"
                  value={configRootInput}
                  onChange={(e) => setConfigRootInput(e.target.value)}
                  placeholder="~/.claude"
                  style={{ flex: 1 }}
                />
                <Button variant="ghost" size="sm" onClick={() => void handlePickConfigRoot()}>
                  Browse
                </Button>
              </div>
            </div>
            <div className="settings-field-group">
              <label className="settings-label">Safety mode</label>
              <select
                className="settings-input"
                value={safetyModeInput}
                onChange={(e) => setSafetyModeInput(e.target.value)}
              >
                <option value="">Default</option>
                <option value="cautious">cautious</option>
                <option value="autonomous">autonomous</option>
                <option value="yolo">yolo</option>
              </select>
            </div>
            <div className="settings-field-group">
              <label className="settings-label">Binary path</label>
              <Input
                className="settings-input"
                type="text"
                value={binaryPathInput}
                onChange={(e) => setBinaryPathInput(e.target.value)}
                placeholder="/usr/local/bin/claude"
              />
            </div>
            <div className="settings-form-actions">
              <Button
                variant="terminal"
                size="sm"
                onClick={() => void handleSave()}
                disabled={saving || !labelInput.trim()}
              >
                {saving ? "Saving..." : "Save"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleCancelEdit}
                disabled={saving}
              >
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <>
            <div className="settings-account-label-row">
              <span className="settings-account-label">{account.label}</span>
              {account.is_default && (
                <Badge className="settings-account-default-badge">default</Badge>
              )}
              <Badge variant="outline" style={{ fontSize: "0.65rem", opacity: 0.7 }}>{account.agent_kind}</Badge>
            </div>
            {account.config_root && (
              <span className="settings-account-binary" title="Config folder">{account.config_root}</span>
            )}
            {account.binary_path && (
              <span className="settings-account-binary" title="Binary path">{account.binary_path}</span>
            )}
          </>
        )}
      </div>
      {!editing && (
        <div className="settings-account-actions">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setEditing(true)}
            title="Edit account"
          >
            Edit
          </Button>
          {!account.is_default && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void handleSetDefault()}
              disabled={saving || deleting}
              title="Set as default"
            >
              Set default
            </Button>
          )}
          {confirmDelete ? (
            <>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void handleDelete()}
                disabled={deleting}
                style={{ color: "var(--destructive, #e55)" }}
              >
                {deleting ? "Removing..." : "Confirm remove"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setConfirmDelete(false)}
                disabled={deleting}
              >
                Cancel
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void handleDelete()}
              disabled={deleting || saving}
              title="Remove account"
            >
              Remove
            </Button>
          )}
        </div>
      )}
    </Card>
  );
}

// --- Appearance Section ---

const THEMES = [
  { id: "default", label: "Mission Control", description: "Warm amber NASA console with industrial bezels and CRT glow" },
  { id: "ice", label: "Ice Station", description: "Cold blue-cyan tactical display" },
  { id: "ember", label: "Ember", description: "Hot red-orange industrial console" },
  { id: "verdant", label: "Verdant", description: "Deep green forest terminal" },
  { id: "ultraviolet", label: "Ultraviolet", description: "Purple-magenta neon console" },
] as const;

const FONT_OPTIONS = [
  { value: "", label: "Theme default" },
  { value: '"IBM Plex Sans", system-ui, sans-serif', label: "IBM Plex Sans" },
  { value: '"Inter", system-ui, sans-serif', label: "Inter" },
  { value: '"Segoe UI", system-ui, sans-serif', label: "Segoe UI" },
  { value: 'system-ui, sans-serif', label: "System UI" },
  { value: '"JetBrains Mono", monospace', label: "JetBrains Mono" },
  { value: '"Fira Sans", system-ui, sans-serif', label: "Fira Sans" },
];

const MONO_OPTIONS = [
  { value: "", label: "Theme default" },
  { value: '"JetBrains Mono", monospace', label: "JetBrains Mono" },
  { value: '"Fira Code", monospace', label: "Fira Code" },
  { value: '"Cascadia Code", monospace', label: "Cascadia Code" },
  { value: '"IBM Plex Mono", monospace', label: "IBM Plex Mono" },
  { value: '"Source Code Pro", monospace', label: "Source Code Pro" },
  { value: '"Consolas", monospace', label: "Consolas" },
];

const ADVANCED_TOKEN_GROUPS = [
  {
    label: "Surfaces",
    tokens: [
      { var: "--surface-base", label: "Base" },
      { var: "--surface-raised", label: "Raised" },
      { var: "--surface-overlay", label: "Overlay" },
      { var: "--surface-sunken", label: "Sunken" },
    ],
  },
  {
    label: "Text",
    tokens: [
      { var: "--text-primary", label: "Primary" },
      { var: "--text-secondary", label: "Secondary" },
      { var: "--text-tertiary", label: "Tertiary" },
    ],
  },
  {
    label: "Borders",
    tokens: [
      { var: "--border-subtle", label: "Subtle" },
      { var: "--border-default", label: "Default" },
    ],
  },
  {
    label: "Status",
    tokens: [
      { var: "--color-success", label: "Success" },
      { var: "--color-warning", label: "Warning" },
      { var: "--color-error", label: "Error" },
      { var: "--color-info", label: "Info" },
    ],
  },
];

function useAppearance() {
  const [overrides, setOverrides] = useState<AppearanceOverrides>(loadOverrides);

  function update(patch: Partial<AppearanceOverrides>) {
    setOverrides((prev) => {
      const next = { ...prev, ...patch };
      saveOverrides(next);
      applyOverrides(next);
      return next;
    });
  }

  function setToken(varName: string, value: string) {
    setOverrides((prev) => {
      const tokens = { ...prev.tokens };
      if (value) {
        tokens[varName] = value;
      } else {
        delete tokens[varName];
      }
      const next = { ...prev, tokens };
      saveOverrides(next);
      applyOverrides(next);
      return next;
    });
  }

  function reset() {
    resetOverrides();
    setOverrides({ brightness: 1.0, fontScale: 1.0, fontSans: "", fontMono: "", accentColor: "", uiDensity: "default", tokens: {} });
  }

  function reloadForTheme(themeId: string) {
    const themeOverrides = loadOverrides(themeId);
    setOverrides(themeOverrides);
    applyOverrides(themeOverrides);
  }

  return { overrides, update, setToken, reset, reloadForTheme };
}

/** Read the computed value of a CSS variable from the current theme (ignoring inline overrides). */
function getThemeTokenValue(varName: string): string {
  // Temporarily read from computed style — this includes inline overrides,
  // but for color inputs we just need a reasonable starting value
  return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
}

function AppearanceSection() {
  const [currentTheme, setCurrentTheme] = useState(
    () => document.documentElement.dataset.theme ?? "default",
  );
  const { overrides, update, setToken, reset, reloadForTheme } = useAppearance();
  const [showAdvanced, setShowAdvanced] = useState(false);

  const hasOverrides =
    overrides.brightness !== 1.0 ||
    overrides.fontScale !== 1.0 ||
    overrides.fontSans !== "" ||
    overrides.fontMono !== "" ||
    overrides.accentColor !== "" ||
    overrides.uiDensity !== "default" ||
    Object.keys(overrides.tokens).length > 0;

  function applyTheme(theme: string) {
    document.documentElement.dataset.theme = theme;
    setStoredValue("emery.theme", theme, "euri.theme");
    setCurrentTheme(theme);
    // Load and apply per-theme overrides
    reloadForTheme(theme);
  }

  return (
    <div className="appearance-section">
      {/* Theme picker */}
      <Card className="settings-panel">
        <CardHeader>
          <CardTitle className="settings-section-title">Theme</CardTitle>
          <CardDescription>Choose the active shell palette and visual treatment.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="settings-theme-cards">
            {THEMES.map((theme) => (
              <Button
                key={theme.id}
                variant={currentTheme === theme.id ? "default" : "ghost"}
                className={`settings-theme-card h-auto flex-col items-start px-4 py-4 ${currentTheme === theme.id ? " active" : ""}`}
                onClick={() => applyTheme(theme.id)}
                data-theme-preview={theme.id}
              >
                <span className="settings-theme-card-name">{theme.label}</span>
                <span className="settings-theme-card-desc">{theme.description}</span>
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Customization controls */}
      <Card className="settings-panel">
        <CardHeader>
          <div className="settings-panel-header-row">
            <div>
              <CardTitle className="settings-section-title">Customization</CardTitle>
              <CardDescription>Override theme defaults. Changes apply live.</CardDescription>
            </div>
            {hasOverrides && (
              <Button variant="ghost" size="sm" onClick={reset}>
                Reset all
              </Button>
            )}
          </div>
        </CardHeader>
        <CardContent>
          <div className="appearance-controls">
            {/* Brightness */}
            <div className="appearance-control-row">
              <label className="settings-label">Brightness</label>
              <div className="appearance-slider-row">
                <input
                  type="range"
                  className="appearance-slider"
                  min="0.5"
                  max="1.5"
                  step="0.05"
                  value={overrides.brightness}
                  onChange={(e) => update({ brightness: parseFloat(e.target.value) })}
                />
                <span className="appearance-slider-value">{Math.round(overrides.brightness * 100)}%</span>
              </div>
            </div>

            {/* Font size */}
            <div className="appearance-control-row">
              <label className="settings-label">Font size</label>
              <div className="appearance-slider-row">
                <input
                  type="range"
                  className="appearance-slider"
                  min="0.75"
                  max="1.5"
                  step="0.05"
                  value={overrides.fontScale}
                  onChange={(e) => update({ fontScale: parseFloat(e.target.value) })}
                />
                <span className="appearance-slider-value">{Math.round(overrides.fontScale * 100)}%</span>
              </div>
            </div>

            {/* Font family */}
            <div className="appearance-control-row">
              <label className="settings-label">UI font</label>
              <Select
                className="settings-select"
                value={overrides.fontSans}
                onChange={(e) => update({ fontSans: e.target.value })}
              >
                {FONT_OPTIONS.map((f) => (
                  <option key={f.value} value={f.value}>{f.label}</option>
                ))}
              </Select>
            </div>

            {/* Mono font */}
            <div className="appearance-control-row">
              <label className="settings-label">Mono font</label>
              <Select
                className="settings-select"
                value={overrides.fontMono}
                onChange={(e) => update({ fontMono: e.target.value })}
              >
                {MONO_OPTIONS.map((f) => (
                  <option key={f.value} value={f.value}>{f.label}</option>
                ))}
              </Select>
            </div>

            {/* Accent color */}
            <div className="appearance-control-row">
              <label className="settings-label">Accent color</label>
              <div className="appearance-color-row">
                <input
                  type="color"
                  className="appearance-color-input"
                  value={overrides.accentColor || getThemeTokenValue("--accent") || "#d8a25a"}
                  onChange={(e) => update({ accentColor: e.target.value })}
                />
                <span className="appearance-color-hex">
                  {overrides.accentColor || "Theme default"}
                </span>
                {overrides.accentColor && (
                  <Button variant="ghost" size="sm" onClick={() => update({ accentColor: "" })}>
                    Reset
                  </Button>
                )}
              </div>
            </div>

            {/* UI density */}
            <div className="appearance-control-row">
              <label className="settings-label">UI density</label>
              <div className="appearance-density-row">
                {(["compact", "default", "comfortable"] as const).map((d) => (
                  <Button
                    key={d}
                    variant={overrides.uiDensity === d ? "default" : "ghost"}
                    size="sm"
                    className="appearance-density-btn"
                    onClick={() => update({ uiDensity: d })}
                  >
                    {d.charAt(0).toUpperCase() + d.slice(1)}
                  </Button>
                ))}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Advanced token overrides */}
      <Card className="settings-panel">
        <CardHeader>
          <button
            className="appearance-advanced-toggle"
            onClick={() => setShowAdvanced((v) => !v)}
          >
            <span className="settings-section-title" style={{ cursor: "pointer" }}>
              Advanced
            </span>
            <span className="appearance-advanced-arrow" data-open={showAdvanced}>
              {showAdvanced ? "\u25B4" : "\u25BE"}
            </span>
          </button>
          {!showAdvanced && (
            <CardDescription>Per-token color overrides for surfaces, text, borders, and status.</CardDescription>
          )}
        </CardHeader>
        {showAdvanced && (
          <CardContent>
            <div className="appearance-advanced-groups">
              {ADVANCED_TOKEN_GROUPS.map((group) => (
                <div key={group.label} className="appearance-token-group">
                  <span className="appearance-token-group-label">{group.label}</span>
                  <div className="appearance-token-list">
                    {group.tokens.map((tok) => {
                      const currentVal = overrides.tokens[tok.var] || "";
                      const themeVal = getThemeTokenValue(tok.var) || "#000000";
                      // Normalize to hex for color input
                      const inputVal = currentVal || normalizeToHex(themeVal);
                      return (
                        <div key={tok.var} className="appearance-token-row">
                          <input
                            type="color"
                            className="appearance-color-input appearance-color-input-sm"
                            value={inputVal}
                            onChange={(e) => setToken(tok.var, e.target.value)}
                          />
                          <span className="appearance-token-label">{tok.label}</span>
                          {currentVal && (
                            <button
                              className="appearance-token-reset"
                              onClick={() => setToken(tok.var, "")}
                              title="Reset to theme default"
                            >
                              x
                            </button>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        )}
      </Card>
    </div>
  );
}

/** Best-effort convert CSS color value to hex for color inputs. */
function normalizeToHex(cssColor: string): string {
  if (cssColor.startsWith("#")) {
    // Expand shorthand
    if (cssColor.length === 4) {
      return "#" + cssColor[1] + cssColor[1] + cssColor[2] + cssColor[2] + cssColor[3] + cssColor[3];
    }
    return cssColor.slice(0, 7); // strip alpha if 8-digit
  }
  // Try to parse rgb/rgba
  const m = cssColor.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
  if (m) {
    return "#" + [m[1], m[2], m[3]].map(c => parseInt(c).toString(16).padStart(2, "0")).join("");
  }
  return "#000000";
}

// --- Agent Defaults Section ---

const SAFETY_MODES = [
  { value: "", label: "Cautious (Default)", description: "Agent asks before destructive operations" },
  { value: "cautious", label: "Cautious", description: "Agent asks before destructive operations" },
  { value: "yolo", label: "Autonomous (Yolo)", description: "Agent can read, write, and execute without confirmation" },
];

const KNOWN_MODELS_BY_KIND: Record<string, Array<{ value: string; label: string }>> = {
  claude: [
    { value: "", label: "Sonnet 4.5 (Default)" },
    { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
    { value: "claude-opus-4-6", label: "Claude Opus 4.6" },
    { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
    { value: "claude-sonnet-4-5-20250514", label: "Claude Sonnet 4.5" },
  ],
  codex: [
    { value: "", label: "Codex Mini (Default)" },
    { value: "codex-mini-latest", label: "Codex Mini" },
    { value: "o4-mini", label: "o4-mini" },
    { value: "o3", label: "o3" },
  ],
  gemini: [
    { value: "", label: "Gemini 2.5 Pro (Default)" },
    { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
  ],
};

function getKnownModels(agentKind: string): Array<{ value: string; label: string }> {
  return KNOWN_MODELS_BY_KIND[agentKind] ?? KNOWN_MODELS_BY_KIND["claude"];
}

function AgentDefaultsSection() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const accounts: AccountSummary[] = (bootstrap?.accounts ?? []).filter((a) => a.status !== "disabled");

  return (
    <Card className="settings-panel">
      <CardHeader>
      <CardTitle className="settings-section-title">Agent Defaults</CardTitle>
      <CardDescription className="settings-section-desc">
        Per-account default model and safety mode. Resolution order: built-in defaults &rarr; <strong>account (here)</strong> &rarr; project overrides &rarr; session overrides. See <em>Config Resolution</em> tab for the full hierarchy.
      </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
      {accounts.length === 0 ? (
        <div className="settings-empty-note">No accounts configured.</div>
      ) : (
        <div className="settings-agent-defaults-list">
          {accounts.map((account) => (
            <AgentDefaultsRow key={account.id} account={account} loadingKeys={loadingKeys} />
          ))}
        </div>
      )}
      </CardContent>
    </Card>
  );
}

function AgentDefaultsRow({
  account,
  loadingKeys,
}: {
  account: AccountSummary;
  loadingKeys: Record<string, boolean>;
}) {
  const knownModels = getKnownModels(account.agent_kind);
  const [modelInput, setModelInput] = useState(account.default_model ?? "");
  const [safetyMode, setSafetyMode] = useState(account.default_safety_mode ?? "");
  const [saved, setSaved] = useState(false);
  const [customModelMode, setCustomModelMode] = useState(() => {
    const val = account.default_model ?? "";
    return val !== "" && !knownModels.some((m) => m.value === val);
  });

  // Sync local state when account data changes (e.g. after a save + refresh)
  useEffect(() => {
    const val = account.default_model ?? "";
    setModelInput(val);
    setSafetyMode(account.default_safety_mode ?? "");
    setCustomModelMode(val !== "" && !knownModels.some((m) => m.value === val));
  }, [account.default_model, account.default_safety_mode, knownModels]);

  const saving = loadingKeys[`update-account:${account.id}`] ?? false;

  async function handleSave() {
    await appStore.handleUpdateAccount(account.id, {
      default_model: modelInput.trim() || null,
      default_safety_mode: safetyMode || null,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  function handleModelSelectChange(e: React.ChangeEvent<HTMLSelectElement>) {
    const v = e.target.value;
    if (v === "__custom__") {
      setCustomModelMode(true);
      setModelInput("");
      setSaved(false);
    } else {
      setCustomModelMode(false);
      setModelInput(v);
      setSaved(false);
    }
  }

  return (
    <Card className="settings-agent-defaults-row p-4">
      <div className="settings-agent-defaults-account-name">{account.label}</div>
      <div className="settings-field-group">
        <label className="settings-label">Default model</label>
        {customModelMode ? (
          <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
            <Input
              className="settings-input"
              type="text"
              value={modelInput}
              onChange={(e) => {
                setModelInput(e.target.value);
                setSaved(false);
              }}
              placeholder="e.g. claude-sonnet-4-6"
              style={{ flex: 1 }}
            />
            <Button variant="ghost" size="sm" onClick={() => { setCustomModelMode(false); setModelInput(""); setSaved(false); }}>
              Back
            </Button>
          </div>
        ) : (
          <Select
            className="settings-select"
            value={modelInput}
            onChange={handleModelSelectChange}
          >
            {knownModels.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
            <option value="__custom__">Custom...</option>
          </Select>
        )}
      </div>
      <div className="settings-field-group">
        <label className="settings-label">Safety mode</label>
        <Select
          className="settings-select"
          value={safetyMode}
          onChange={(e) => {
            setSafetyMode(e.target.value);
            setSaved(false);
          }}
        >
          {SAFETY_MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </Select>
        {safetyMode && SAFETY_MODES.find((m) => m.value === safetyMode)?.description && (
          <div style={{ fontSize: "0.72rem", color: "var(--text-secondary)", marginTop: "2px" }}>
            {SAFETY_MODES.find((m) => m.value === safetyMode)!.description}
          </div>
        )}
      </div>
      <div className="settings-form-actions">
        <Button
          variant="terminal"
          size="sm"
          onClick={() => void handleSave()}
          disabled={saving}
        >
          {saving ? "Saving..." : saved ? "Saved" : "Save"}
        </Button>
      </div>
    </Card>
  );
}

// --- GitHub Section ---

// ── MCP Servers Section ──────────────────────────────────────────────────

function McpServersSection() {
  const [servers, setServers] = useState<McpServerSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddForm, setShowAddForm] = useState(false);
  const [addType, setAddType] = useState<"stdio" | "http">("stdio");
  const [addName, setAddName] = useState("");
  const [addCommand, setAddCommand] = useState("");
  const [addArgs, setAddArgs] = useState("");
  const [addUrl, setAddUrl] = useState("");
  const [addError, setAddError] = useState("");
  const [creating, setCreating] = useState(false);

  async function refresh() {
    try {
      const result = await listMcpServers();
      setServers(result);
    } catch { /* ignore */ }
    setLoading(false);
  }

  useEffect(() => { void refresh(); }, []);

  async function handleCreate() {
    setAddError("");
    if (!addName.trim()) { setAddError("Name is required"); return; }
    if (addType === "stdio" && !addCommand.trim()) { setAddError("Command is required for stdio servers"); return; }
    if (addType === "http" && !addUrl.trim()) { setAddError("URL is required for HTTP servers"); return; }

    setCreating(true);
    try {
      const args = addArgs.trim() ? addArgs.split(/\s+/) : undefined;
      await createMcpServer({
        name: addName.trim(),
        server_type: addType,
        command: addType === "stdio" ? addCommand.trim() : null,
        args: addType === "stdio" ? args : undefined,
        url: addType === "http" ? addUrl.trim() : null,
      });
      setShowAddForm(false);
      setAddName(""); setAddCommand(""); setAddArgs(""); setAddUrl("");
      await refresh();
    } catch (err) {
      setAddError(String(err));
    } finally {
      setCreating(false);
    }
  }

  async function handleToggle(server: McpServerSummary) {
    try {
      await updateMcpServer(server.id, { enabled: !server.enabled });
      await refresh();
    } catch { /* ignore */ }
  }

  async function handleDelete(serverId: string) {
    try {
      await deleteMcpServer(serverId);
      await refresh();
    } catch { /* ignore */ }
  }

  if (loading) return <div className="modal-loading">Loading...</div>;

  return (
    <Card>
      <CardHeader>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <CardTitle>MCP Servers</CardTitle>
            <CardDescription>
              MCP servers are injected into every agent session launched through Emery via <code>--mcp-config</code>.
              Stdio servers run a local binary. HTTP servers connect to a remote URL.
            </CardDescription>
          </div>
          {!showAddForm && (
            <Button variant="ghost" size="sm" onClick={() => setShowAddForm(true)}>+ Add</Button>
          )}
        </div>
      </CardHeader>
      <CardContent>
        {showAddForm && (
          <div className="settings-add-form" style={{ marginBottom: 16, padding: 12, border: "1px solid var(--border-primary)", borderRadius: 6 }}>
            <div className="dispatch-field" style={{ marginBottom: 8 }}>
              <label className="dispatch-field-label">Type</label>
              <Select value={addType} onChange={(e) => setAddType(e.target.value as "stdio" | "http")}>
                <option value="stdio">Stdio (local binary)</option>
                <option value="http">HTTP (remote URL)</option>
              </Select>
            </div>
            <div className="dispatch-field" style={{ marginBottom: 8 }}>
              <label className="dispatch-field-label">Name</label>
              <Input value={addName} onChange={(e) => setAddName(e.target.value)} placeholder="e.g. my-tools" />
            </div>
            {addType === "stdio" ? (
              <>
                <div className="dispatch-field" style={{ marginBottom: 8 }}>
                  <label className="dispatch-field-label">Command</label>
                  <div style={{ display: "flex", gap: 4 }}>
                    <Input value={addCommand} onChange={(e) => setAddCommand(e.target.value)} placeholder="Path to MCP server binary" style={{ flex: 1 }} />
                    <Button variant="ghost" size="sm" onClick={async () => {
                      const folder = await pickFolder();
                      if (folder) setAddCommand(folder);
                    }}>Browse</Button>
                  </div>
                </div>
                <div className="dispatch-field" style={{ marginBottom: 8 }}>
                  <label className="dispatch-field-label">Args</label>
                  <Input value={addArgs} onChange={(e) => setAddArgs(e.target.value)} placeholder="Space-separated arguments (optional)" />
                </div>
              </>
            ) : (
              <div className="dispatch-field" style={{ marginBottom: 8 }}>
                <label className="dispatch-field-label">URL</label>
                <Input value={addUrl} onChange={(e) => setAddUrl(e.target.value)} placeholder="https://example.com/mcp" />
              </div>
            )}
            {addError && <p style={{ color: "var(--text-danger)", fontSize: 12, marginBottom: 8 }}>{addError}</p>}
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <Button variant="ghost" size="sm" onClick={() => { setShowAddForm(false); setAddError(""); }}>Cancel</Button>
              <Button variant="default" size="sm" onClick={handleCreate} disabled={creating}>
                {creating ? "Creating..." : "Add Server"}
              </Button>
            </div>
          </div>
        )}

        {servers.length === 0 && !showAddForm && (
          <p style={{ color: "var(--text-secondary)", fontSize: 13 }}>No MCP servers configured. The built-in emery server will be added on next supervisor restart.</p>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {servers.map((server) => (
            <McpServerRow key={server.id} server={server} onToggle={handleToggle} onDelete={handleDelete} />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function McpServerRow({ server, onToggle, onDelete }: {
  server: McpServerSummary;
  onToggle: (s: McpServerSummary) => void;
  onDelete: (id: string) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 8, padding: "8px 12px",
      border: "1px solid var(--border-primary)", borderRadius: 6,
      opacity: server.enabled ? 1 : 0.5,
    }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ fontWeight: 600, fontSize: 13 }}>{server.name}</span>
          <Badge variant={server.server_type === "http" ? "outline" : "secondary"} style={{ fontSize: 10 }}>
            {server.server_type}
          </Badge>
          {server.is_builtin && (
            <Badge variant="outline" style={{ fontSize: 10 }}>built-in</Badge>
          )}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {server.server_type === "http"
            ? server.url ?? "—"
            : server.command ?? "—"
          }
          {server.args_json && server.args_json !== "[]" && (
            <span style={{ marginLeft: 4, opacity: 0.7 }}>{server.args_json}</span>
          )}
        </div>
      </div>
      <Button variant="ghost" size="sm" onClick={() => onToggle(server)}>
        {server.enabled ? "Disable" : "Enable"}
      </Button>
      {!server.is_builtin && (
        confirmDelete ? (
          <Button variant="secondary" size="sm" onClick={() => { onDelete(server.id); setConfirmDelete(false); }}>
            Confirm
          </Button>
        ) : (
          <Button variant="ghost" size="sm" onClick={() => setConfirmDelete(true)}>
            Remove
          </Button>
        )
      )}
    </div>
  );
}

function GitHubSection() {
  const githubToken = useAppStore((s) => s.githubToken);
  const [tokenInput, setTokenInput] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    appStore.loadGithubToken();
  }, []);

  useEffect(() => {
    setTokenInput(appStore.getState().githubToken);
  }, [githubToken]);

  function handleSave() {
    appStore.saveGithubToken(tokenInput.trim());
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  function handleClear() {
    appStore.saveGithubToken("");
    setTokenInput("");
    setSaved(false);
  }

  return (
    <Card className="settings-panel">
      <CardHeader>
      <CardTitle className="settings-section-title">GitHub</CardTitle>
      <CardDescription className="settings-section-desc">
        Personal access token for GitHub integration. Stored in browser local storage.
      </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
      <div className="settings-field-group">
        <label className="settings-label" htmlFor="github-pat">
          Personal Access Token
        </label>
        <div className="settings-input-row">
          <Input
            id="github-pat"
            className="settings-input"
            type={showToken ? "text" : "password"}
            value={tokenInput}
            onChange={(e) => {
              setTokenInput(e.target.value);
              setSaved(false);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSave();
            }}
            placeholder="ghp_..."
            autoComplete="off"
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowToken((v) => !v)}
            title={showToken ? "Hide token" : "Show token"}
          >
            {showToken ? "Hide" : "Show"}
          </Button>
        </div>
      </div>
      <div className="settings-form-actions">
        <Button
          variant="terminal"
          size="sm"
          onClick={handleSave}
          disabled={!tokenInput.trim()}
        >
          {saved ? "Saved" : "Save"}
        </Button>
        {githubToken && (
          <Button variant="ghost" size="sm" onClick={handleClear}>
            Clear
          </Button>
        )}
      </div>
      {githubToken && (
        <div className="settings-github-status">
          Token configured.
        </div>
      )}
      </CardContent>
    </Card>
  );
}

// --- Knowledge Store Section ---

function KnowledgeStoreSection() {
  const knowledgeStoreBackend = useAppStore((s) => s.knowledgeStoreBackend);

  useEffect(() => {
    appStore.loadKnowledgeStoreBackend();
  }, []);

  function handleChange(backend: "embedded" | "wcp_cloud") {
    appStore.saveKnowledgeStoreBackend(backend);
  }

  return (
    <Card className="settings-panel">
      <CardHeader>
        <CardTitle className="settings-section-title">Knowledge Store</CardTitle>
        <CardDescription className="settings-section-desc">
          Choose which knowledge store backend Emery uses for work items, documents, and context.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="settings-field-group">
          <label className="settings-label">Backend</label>
          <div className="knowledge-store-options">
            <label className={`knowledge-store-option${knowledgeStoreBackend === "embedded" ? " selected" : ""}`}>
              <input
                type="radio"
                name="knowledge-store-backend"
                value="embedded"
                checked={knowledgeStoreBackend === "embedded"}
                onChange={() => handleChange("embedded")}
                className="knowledge-store-radio"
              />
              <div className="knowledge-store-option-body">
                <span className="knowledge-store-option-title">Embedded (Local)</span>
                <span className="knowledge-store-option-desc">
                  Use the built-in emery-mcp knowledge base stored on this machine.
                </span>
              </div>
            </label>
            <label className={`knowledge-store-option${knowledgeStoreBackend === "wcp_cloud" ? " selected" : ""}`}>
              <input
                type="radio"
                name="knowledge-store-backend"
                value="wcp_cloud"
                checked={knowledgeStoreBackend === "wcp_cloud"}
                onChange={() => handleChange("wcp_cloud")}
                className="knowledge-store-radio"
              />
              <div className="knowledge-store-option-body">
                <span className="knowledge-store-option-title">WCP Cloud</span>
                <span className="knowledge-store-option-desc">
                  Use the remote Work Context Protocol hosted knowledge store.
                </span>
              </div>
            </label>
          </div>
        </div>

        {knowledgeStoreBackend === "embedded" && (
          <div className="knowledge-store-info knowledge-store-info--embedded">
            Using local knowledge store via emery-mcp. All work items and documents are stored on this machine.
          </div>
        )}
        {knowledgeStoreBackend === "wcp_cloud" && (
          <div className="knowledge-store-info knowledge-store-info--cloud">
            WCP Cloud selected. Configure your connection at{" "}
            <a
              href="https://workcontextprotocol.io"
              target="_blank"
              rel="noopener noreferrer"
              className="knowledge-store-link"
            >
              workcontextprotocol.io
            </a>
            .
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// --- Config Resolution Reference Section ---

function ConfigResolutionSection() {
  return (
    <Card className="settings-panel">
      <CardHeader>
        <CardTitle className="settings-section-title">Config Resolution Reference</CardTitle>
        <CardDescription className="settings-section-desc">
          How Emery resolves configuration variables when launching a session. Higher steps override lower ones.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">

        <div className="config-resolution-reference">
          <div className="config-resolution-tier">
            <div className="config-resolution-tier-header">
              <span className="config-resolution-tier-step">1</span>
              <span className="config-resolution-tier-name">Built-in defaults</span>
              <span className="config-resolution-tier-scope">lowest priority</span>
            </div>
            <div className="config-resolution-tier-body">
              <p>Applied before any user configuration. Origin-mode defaults:</p>
              <ul className="config-resolution-list">
                <li><code>planning</code>, <code>research</code>, <code>dispatch</code> &rarr; <strong>opus</strong></li>
                <li><code>execution</code>, <code>follow_up</code> &rarr; <strong>sonnet</strong></li>
                <li>safety_mode &rarr; <strong>cautious</strong></li>
              </ul>
            </div>
          </div>

          <div className="config-resolution-tier-connector">&darr; overridden by</div>

          <div className="config-resolution-tier">
            <div className="config-resolution-tier-header">
              <span className="config-resolution-tier-step">2</span>
              <span className="config-resolution-tier-name">Account defaults</span>
              <span className="config-resolution-tier-scope">global</span>
            </div>
            <div className="config-resolution-tier-body">
              <p>Set in <strong>Settings &rarr; Agent Defaults</strong>. Applies to all sessions using that account.</p>
              <ul className="config-resolution-list">
                <li><code>default_model</code> &mdash; overrides origin-mode built-in model</li>
                <li><code>default_safety_mode</code> &mdash; overrides built-in safety default</li>
                <li><code>default_launch_args_json</code> &mdash; extra CLI args</li>
              </ul>
            </div>
          </div>

          <div className="config-resolution-tier-connector">&darr; overridden by</div>

          <div className="config-resolution-tier">
            <div className="config-resolution-tier-header">
              <span className="config-resolution-tier-step">3</span>
              <span className="config-resolution-tier-name">Project overrides</span>
              <span className="config-resolution-tier-scope">per-project</span>
            </div>
            <div className="config-resolution-tier-body">
              <p>Set in <strong>Project Settings &rarr; Model Defaults / Safety Overrides</strong>.</p>
              <ul className="config-resolution-list">
                <li><code>model_defaults_json.by_origin_mode</code> &mdash; per-mode model override</li>
                <li><code>model_defaults_json.default</code> &mdash; fallback model for all modes</li>
                <li><code>agent_safety_overrides_json[agent_kind].safety_mode</code> &mdash; safety override</li>
                <li><code>agent_safety_overrides_json[agent_kind].extra_args</code> &mdash; extra CLI args override</li>
              </ul>
            </div>
          </div>

          <div className="config-resolution-tier-connector">&darr; overridden by</div>

          <div className="config-resolution-tier config-resolution-tier-highest">
            <div className="config-resolution-tier-header">
              <span className="config-resolution-tier-step">4</span>
              <span className="config-resolution-tier-name">Session-level overrides</span>
              <span className="config-resolution-tier-scope">highest priority</span>
            </div>
            <div className="config-resolution-tier-body">
              <p>Passed directly when dispatching a session (e.g. via MCP <code>emery_session_create</code>).</p>
              <ul className="config-resolution-list">
                <li><code>safety_mode</code> &mdash; explicit safety override for this session</li>
                <li><code>model</code> &mdash; explicit model override for this session</li>
                <li><code>extra_args</code> &mdash; explicit extra CLI args for this session</li>
              </ul>
            </div>
          </div>
        </div>

        <div className="config-resolution-note">
          The <strong>Config Preview</strong> in each project&apos;s settings shows what a session would receive
          given the current account and project configuration.
        </div>

      </CardContent>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// API Keys Section
// ---------------------------------------------------------------------------

// Canonical key name for downstream consumers (EMERY-217.002 reads this).
const API_KEY_SLOTS: Array<{ key: string; label: string; description: string }> = [
  { key: "VOYAGE_API_KEY", label: "Voyage AI", description: "Used for generating embeddings (semantic search)" },
];

const VAULT_SCOPE = "global";

function maskValue(value: string): string {
  if (value.length <= 4) return "••••••••";
  return `••••••••${value.slice(-4)}`;
}

function ApiKeysSection() {
  const [lockStatus, setLockStatus] = useState<VaultLockStatus | null>(null);
  const [entries, setEntries] = useState<VaultEntry[]>([]);
  const [maskedValues, setMaskedValues] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [unlocking, setUnlocking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setError(null);
    try {
      const status = await vaultStatus();
      setLockStatus(status);
      if (status.unlocked) {
        const all = await vaultList(VAULT_SCOPE);
        setEntries(all);
        // Fetch masked values for known slots that are stored.
        const masked: Record<string, string> = {};
        for (const slot of API_KEY_SLOTS) {
          const entry = all.find((e) => e.scope === VAULT_SCOPE && e.key === slot.key);
          if (entry) {
            try {
              const result = await vaultGet(VAULT_SCOPE, slot.key);
              if (result.value) {
                masked[slot.key] = maskValue(result.value);
              }
            } catch {
              // If we can't fetch the value, show a generic mask.
              masked[slot.key] = "••••••••";
            }
          }
        }
        setMaskedValues(masked);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function handleUnlock() {
    setUnlocking(true);
    setError(null);
    try {
      await vaultUnlock();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setUnlocking(false);
    }
  }

  function findEntry(key: string): VaultEntry | undefined {
    return entries.find((e) => e.scope === VAULT_SCOPE && e.key === key);
  }

  return (
    <Card className="settings-panel">
      <CardHeader>
        <CardTitle className="settings-section-title">API Keys</CardTitle>
        <CardDescription className="settings-section-desc">
          Third-party API keys stored securely in the encrypted vault.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {loading && <div className="settings-empty-note">Loading…</div>}

        {!loading && error && (
          <div className="settings-empty-note" style={{ color: "var(--destructive, #e55)" }}>
            {error}
          </div>
        )}

        {!loading && !error && lockStatus && !lockStatus.unlocked && (
          <div className="settings-vault-locked">
            <p className="settings-vault-locked-msg">
              The vault is locked. Unlock it to view or edit API keys.
            </p>
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleUnlock()}
              disabled={unlocking}
            >
              {unlocking ? "Unlocking…" : "Unlock Vault"}
            </Button>
          </div>
        )}

        {!loading && !error && lockStatus?.unlocked && (
          <div className="settings-api-key-list">
            {API_KEY_SLOTS.map((slot) => (
              <ApiKeyRow
                key={slot.key}
                slotKey={slot.key}
                label={slot.label}
                description={slot.description}
                entry={findEntry(slot.key)}
                maskedValue={maskedValues[slot.key] ?? null}
                onSaved={() => void refresh()}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function ApiKeyRow({
  slotKey,
  label,
  description,
  entry,
  maskedValue,
  onSaved,
}: {
  slotKey: string;
  label: string;
  description: string;
  entry: VaultEntry | undefined;
  maskedValue: string | null;
  onSaved: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  async function handleSave() {
    if (!inputValue.trim()) return;
    setSaving(true);
    setSaveError(null);
    try {
      await vaultSet(VAULT_SCOPE, slotKey, inputValue.trim(), description);
      setInputValue("");
      setEditing(false);
      onSaved();
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!entry) return;
    setDeleting(true);
    try {
      await vaultDelete(entry.id);
      onSaved();
    } catch {
      // ignore; onSaved refresh will reconcile
    } finally {
      setDeleting(false);
    }
  }

  function handleCancel() {
    setInputValue("");
    setSaveError(null);
    setEditing(false);
  }

  return (
    <div className="settings-api-key-row">
      <div className="settings-api-key-info">
        <span className="settings-api-key-label">{label}</span>
        <span className="settings-api-key-desc">{description}</span>
      </div>
      {editing ? (
        <div className="settings-api-key-edit">
          <Input
            className="settings-input"
            type="password"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSave();
              if (e.key === "Escape") handleCancel();
            }}
            placeholder="Paste your API key here"
            autoFocus
          />
          {saveError && (
            <span className="settings-api-key-error">{saveError}</span>
          )}
          <div className="settings-form-actions">
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleSave()}
              disabled={saving || !inputValue.trim()}
            >
              {saving ? "Saving…" : "Save"}
            </Button>
            <Button variant="ghost" size="sm" onClick={handleCancel} disabled={saving}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="settings-api-key-value-row">
          <span className="settings-api-key-masked">
            {maskedValue ?? <span style={{ opacity: 0.5 }}>not set</span>}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setEditing(true)}
            title={entry ? "Update key" : "Set key"}
          >
            {entry ? "Edit" : "Set"}
          </Button>
          {entry && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void handleDelete()}
              disabled={deleting}
              title="Remove key"
            >
              {deleting ? "Removing…" : "Delete"}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
