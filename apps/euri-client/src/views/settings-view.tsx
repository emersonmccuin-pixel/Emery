import { useEffect, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import type { AccountSummary } from "../types";

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
    <div className="content-frame">
    <div className="global-settings-view">
      <div className="global-settings-header">
        <h2 className="global-settings-title">Settings</h2>
        <button
          className="btn-ghost btn-sm"
          onClick={() => navStore.goBack()}
        >
          ← Back
        </button>
      </div>
      <div className="global-settings-body">
        <nav className="global-settings-sidebar">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              className={`global-settings-nav-item${activeTab === tab.id ? " active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
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

  const accounts: AccountSummary[] = bootstrap?.accounts ?? [];
  const creating = loadingKeys["create-account"] ?? false;

  async function handleCreate() {
    if (!newLabel.trim()) return;
    await appStore.handleCreateAccount({
      label: newLabel.trim(),
      binary_path: newBinaryPath.trim() || null,
    });
    setNewLabel("");
    setNewBinaryPath("");
    setShowAddForm(false);
  }

  return (
    <div className="settings-panel">
      <div className="settings-panel-header-row">
        <h3 className="settings-section-title">Accounts</h3>
        <button
          className="section-add-btn"
          onClick={() => setShowAddForm((v) => !v)}
          title="Add account"
        >
          +
        </button>
      </div>

      {showAddForm && (
        <div className="settings-add-form">
          <div className="settings-field-group">
            <label className="settings-label">Label</label>
            <input
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
            <label className="settings-label">Binary path (optional)</label>
            <input
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
            <button
              className="btn-primary btn-sm"
              onClick={() => void handleCreate()}
              disabled={creating || !newLabel.trim()}
            >
              {creating ? "Creating..." : "Create"}
            </button>
            <button
              className="btn-ghost btn-sm"
              onClick={() => setShowAddForm(false)}
              disabled={creating}
            >
              Cancel
            </button>
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
    </div>
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

  const saving = loadingKeys[`update-account:${account.id}`] ?? false;

  async function handleSaveLabel() {
    if (!labelInput.trim() || labelInput.trim() === account.label) {
      setEditing(false);
      return;
    }
    await appStore.handleUpdateAccount(account.id, { label: labelInput.trim() });
    setEditing(false);
  }

  async function handleSetDefault() {
    if (account.is_default) return;
    await appStore.handleUpdateAccount(account.id, { is_default: true });
  }

  return (
    <div className="settings-account-row">
      <div className="settings-account-info">
        {editing ? (
          <div className="settings-input-row">
            <input
              className="settings-input"
              type="text"
              value={labelInput}
              onChange={(e) => setLabelInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveLabel();
                if (e.key === "Escape") {
                  setLabelInput(account.label);
                  setEditing(false);
                }
              }}
              autoFocus
            />
            <button
              className="btn-primary btn-sm"
              onClick={() => void handleSaveLabel()}
              disabled={saving || !labelInput.trim()}
            >
              {saving ? "..." : "Save"}
            </button>
            <button
              className="btn-ghost btn-sm"
              onClick={() => {
                setLabelInput(account.label);
                setEditing(false);
              }}
              disabled={saving}
            >
              Cancel
            </button>
          </div>
        ) : (
          <div className="settings-account-label-row">
            <span className="settings-account-label">{account.label}</span>
            {account.is_default && (
              <span className="settings-account-default-badge">default</span>
            )}
          </div>
        )}
        {account.binary_path && (
          <span className="settings-account-binary">{account.binary_path}</span>
        )}
        <span className="settings-account-kind">{account.agent_kind}</span>
      </div>
      <div className="settings-account-actions">
        {!editing && (
          <button
            className="btn-ghost btn-sm"
            onClick={() => setEditing(true)}
            title="Edit label"
          >
            Edit
          </button>
        )}
        {!account.is_default && (
          <button
            className="btn-ghost btn-sm"
            onClick={() => void handleSetDefault()}
            disabled={saving}
            title="Set as default"
          >
            Set default
          </button>
        )}
      </div>
    </div>
  );
}

// --- Appearance Section ---

const THEMES = [
  { id: "vaporwave", label: "Vaporwave", description: "Neon purple and cyan on dark" },
  { id: "neutral-dark", label: "Neutral Dark", description: "Clean blue-grey on dark" },
] as const;

function AppearanceSection() {
  const [currentTheme, setCurrentTheme] = useState(
    () => document.documentElement.dataset.theme ?? "vaporwave",
  );

  function applyTheme(theme: string) {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("euri.theme", theme);
    setCurrentTheme(theme);
  }

  return (
    <div className="settings-panel">
      <h3 className="settings-section-title">Appearance</h3>
      <div className="settings-field-group">
        <label className="settings-label">Theme</label>
        <div className="settings-theme-cards">
          {THEMES.map((theme) => (
            <button
              key={theme.id}
              className={`settings-theme-card${currentTheme === theme.id ? " active" : ""}`}
              onClick={() => applyTheme(theme.id)}
              data-theme-preview={theme.id}
            >
              <span className="settings-theme-card-name">{theme.label}</span>
              <span className="settings-theme-card-desc">{theme.description}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

// --- Agent Defaults Section ---

const SAFETY_MODES = [
  { value: "", label: "Default" },
  { value: "full", label: "Full" },
  { value: "permissive", label: "Permissive" },
  { value: "none", label: "None" },
];

function AgentDefaultsSection() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const accounts: AccountSummary[] = bootstrap?.accounts ?? [];

  return (
    <div className="settings-panel">
      <h3 className="settings-section-title">Agent Defaults</h3>
      <p className="settings-section-desc">
        Per-account default model and safety mode. These apply when launching sessions without explicit overrides.
      </p>
      {accounts.length === 0 ? (
        <div className="settings-empty-note">No accounts configured.</div>
      ) : (
        <div className="settings-agent-defaults-list">
          {accounts.map((account) => (
            <AgentDefaultsRow key={account.id} account={account} loadingKeys={loadingKeys} />
          ))}
        </div>
      )}
    </div>
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

  const saving = loadingKeys[`update-account:${account.id}`] ?? false;

  async function handleSave() {
    await appStore.handleUpdateAccount(account.id, {
      default_model: modelInput.trim() || null,
      default_safety_mode: safetyMode || null,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="settings-agent-defaults-row">
      <div className="settings-agent-defaults-account-name">{account.label}</div>
      <div className="settings-field-group">
        <label className="settings-label">Default model</label>
        <input
          className="settings-input"
          type="text"
          value={modelInput}
          onChange={(e) => {
            setModelInput(e.target.value);
            setSaved(false);
          }}
          placeholder="e.g. claude-opus-4-5"
        />
      </div>
      <div className="settings-field-group">
        <label className="settings-label">Safety mode</label>
        <select
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
        </select>
      </div>
      <div className="settings-form-actions">
        <button
          className="btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={saving}
        >
          {saving ? "Saving..." : saved ? "Saved" : "Save"}
        </button>
      </div>
    </div>
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
    <div className="settings-panel">
      <h3 className="settings-section-title">GitHub</h3>
      <p className="settings-section-desc">
        Personal access token for GitHub integration. Stored in browser local storage.
      </p>
      <div className="settings-field-group">
        <label className="settings-label" htmlFor="github-pat">
          Personal Access Token
        </label>
        <div className="settings-input-row">
          <input
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
          <button
            className="btn-ghost btn-sm"
            onClick={() => setShowToken((v) => !v)}
            title={showToken ? "Hide token" : "Show token"}
          >
            {showToken ? "Hide" : "Show"}
          </button>
        </div>
      </div>
      <div className="settings-form-actions">
        <button
          className="btn-primary btn-sm"
          onClick={handleSave}
          disabled={!tokenInput.trim()}
        >
          {saved ? "Saved" : "Save"}
        </button>
        {githubToken && (
          <button className="btn-ghost btn-sm" onClick={handleClear}>
            Clear
          </button>
        )}
      </div>
      {githubToken && (
        <div className="settings-github-status">
          Token configured.
        </div>
      )}
    </div>
  );
}
