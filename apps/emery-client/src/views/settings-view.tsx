import { useEffect, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import type { AccountSummary } from "../types";
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
import { pickFolder } from "../lib";

type SettingsTab = "accounts" | "appearance" | "agent-defaults" | "github";

export function SettingsView() {
  const [activeTab, setActiveTab] = useState<SettingsTab>("accounts");

  const tabs: Array<{ id: SettingsTab; label: string }> = [
    { id: "accounts", label: "Accounts" },
    { id: "appearance", label: "Appearance" },
    { id: "agent-defaults", label: "Agent Defaults" },
    { id: "github", label: "GitHub" },
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
            {activeTab === "github" && <GitHubSection />}
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
    });
    setNewLabel("");
    setNewBinaryPath("");
    setNewConfigRoot("");
    setNewAgentKind("claude");
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
  const [confirmDelete, setConfirmDelete] = useState(false);

  const saving = loadingKeys[`update-account:${account.id}`] ?? false;
  const deleting = loadingKeys[`delete-account:${account.id}`] ?? false;

  async function handleSave() {
    if (!labelInput.trim()) return;
    await appStore.handleUpdateAccount(account.id, {
      label: labelInput.trim(),
      binary_path: binaryPathInput.trim() || null,
      config_root: configRootInput.trim() || null,
    });
    setEditing(false);
  }

  function handleCancelEdit() {
    setLabelInput(account.label);
    setBinaryPathInput(account.binary_path ?? "");
    setConfigRootInput(account.config_root ?? "");
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
  { id: "cyberpunk", label: "Cyberpunk", description: "Glitched neon HUD with scanlines and acid green" },
  { id: "fallout", label: "Fallout", description: "Pip-Boy green phosphor CRT with flicker and Vault-Tec ASCII" },
  { id: "vapor", label: "Vaporwave", description: "Sunset gradient with city skyline silhouette" },
  { id: "synthwave", label: "Synthwave", description: "80s neon grid with hot pink horizon and glowing sun" },
  { id: "deep-ocean", label: "Deep Ocean", description: "Bioluminescent abyss with drifting particles and caustics" },
  { id: "aurora", label: "Aurora", description: "Northern lights with twinkling stars and shifting color bands" },
  { id: "noir", label: "Noir", description: "Film noir warmth with venetian blind light and gold accents" },
  { id: "amber", label: "Amber Terminal", description: "Classic amber phosphor CRT with dot grid and vignette" },
  { id: "mars", label: "Mars Colony", description: "Dusty red industrial with hab-module readout" },
  { id: "neutral-dark", label: "Neutral Dark", description: "Clean blue-grey on dark" },
] as const;

function AppearanceSection() {
  const [currentTheme, setCurrentTheme] = useState(
    () => document.documentElement.dataset.theme ?? "cyberpunk",
  );

  function applyTheme(theme: string) {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("emery.theme", theme);
    localStorage.removeItem("euri.theme");
    setCurrentTheme(theme);
  }

  return (
    <Card className="settings-panel">
      <CardHeader>
        <CardTitle className="settings-section-title">Appearance</CardTitle>
        <CardDescription>Choose the active shell palette and visual treatment.</CardDescription>
      </CardHeader>
      <CardContent>
      <div className="settings-field-group">
        <label className="settings-label">Theme</label>
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
      </div>
      </CardContent>
    </Card>
  );
}

// --- Agent Defaults Section ---

const SAFETY_MODES = [
  { value: "", label: "Default", description: "" },
  { value: "full", label: "Autonomous", description: "Agent can read, write, and execute without confirmation" },
  { value: "normal", label: "Supervised", description: "Agent asks before destructive operations" },
  { value: "restricted", label: "Read Only", description: "Agent can read files but cannot write or execute" },
];

const KNOWN_MODELS = [
  { value: "", label: "Default" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
  { value: "claude-opus-4-6", label: "Claude Opus 4.6" },
  { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
  { value: "claude-sonnet-4-5-20250514", label: "Claude Sonnet 4.5" },
];

function AgentDefaultsSection() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const accounts: AccountSummary[] = (bootstrap?.accounts ?? []).filter((a) => a.status !== "disabled");

  return (
    <Card className="settings-panel">
      <CardHeader>
      <CardTitle className="settings-section-title">Agent Defaults</CardTitle>
      <CardDescription className="settings-section-desc">
        Per-account default model and safety mode. These apply when launching sessions without explicit overrides.
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
  const [modelInput, setModelInput] = useState(account.default_model ?? "");
  const [safetyMode, setSafetyMode] = useState(account.default_safety_mode ?? "");
  const [saved, setSaved] = useState(false);
  const [customModelMode, setCustomModelMode] = useState(() => {
    const val = account.default_model ?? "";
    return val !== "" && !KNOWN_MODELS.some((m) => m.value === val);
  });

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
            {KNOWN_MODELS.map((m) => (
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
