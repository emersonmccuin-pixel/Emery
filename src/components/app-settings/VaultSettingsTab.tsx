import { useEffect, useState, type FormEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { invoke } from "@/lib/tauri";
import type { VaultEntryRecord, VaultSnapshot } from "@/types";

const EMPTY_VAULT_FORM = {
  id: null as number | null,
  name: "",
  kind: "token",
  description: "",
  scopeTags: "",
  gatePolicy: "confirm_session",
  value: "",
};

function parseScopeTags(value: string) {
  return value
    .split(/[\n,]/)
    .map((token) => token.trim())
    .filter(Boolean);
}

function formatGatePolicyLabel(policy: string) {
  switch (policy) {
    case "auto":
      return "Auto";
    case "confirm_each_use":
      return "Confirm Each Use";
    case "confirm_session":
    default:
      return "Confirm Per Session";
  }
}

function vaultDiagnosticsArgs(form: typeof EMPTY_VAULT_FORM) {
  return {
    input: {
      id: form.id,
      name: form.name,
      kind: form.kind,
      description: form.description,
      scopeTags: parseScopeTags(form.scopeTags),
      gatePolicy: form.gatePolicy,
      value: form.value ? "<redacted>" : undefined,
    },
  };
}

function VaultSettingsTab() {
  const [snapshot, setSnapshot] = useState<VaultSnapshot | null>(null);
  const [form, setForm] = useState(EMPTY_VAULT_FORM);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [activeDeleteId, setActiveDeleteId] = useState<number | null>(null);

  useEffect(() => {
    void loadVaultEntries();
  }, []);

  async function loadVaultEntries() {
    setIsLoading(true);
    setError(null);

    try {
      const next = await invoke<VaultSnapshot>("list_vault_entries");
      setSnapshot(next);
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to load vault entries",
      );
    } finally {
      setIsLoading(false);
    }
  }

  function startEdit(entry: VaultEntryRecord) {
    setForm({
      id: entry.id,
      name: entry.name,
      kind: entry.kind,
      description: entry.description,
      scopeTags: entry.scopeTags.join(", "),
      gatePolicy: entry.gatePolicy,
      value: "",
    });
    setMessage(null);
    setError(null);
  }

  function resetForm() {
    setForm(EMPTY_VAULT_FORM);
  }

  async function submitVaultEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setError(null);
    setMessage(null);

    try {
      const next = await invoke<VaultSnapshot>(
        "upsert_vault_entry",
        {
          input: {
            id: form.id,
            name: form.name,
            kind: form.kind,
            description: form.description,
            scopeTags: parseScopeTags(form.scopeTags),
            gatePolicy: form.gatePolicy,
            value: form.value || undefined,
          },
        },
        { diagnosticsArgs: vaultDiagnosticsArgs(form) },
      );

      setSnapshot(next);
      setMessage(
        form.id === null
          ? "Vault entry saved to Stronghold."
          : form.value
            ? "Vault entry updated and rotated."
            : "Vault entry metadata updated.",
      );
      resetForm();
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to save vault entry",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function deleteEntry(entry: VaultEntryRecord) {
    if (!confirm(`Delete vault entry "${entry.name}"?`)) {
      return;
    }

    setActiveDeleteId(entry.id);
    setError(null);
    setMessage(null);

    try {
      const next = await invoke<VaultSnapshot>("delete_vault_entry", {
        input: { id: entry.id },
      });
      setSnapshot(next);
      if (form.id === entry.id) {
        resetForm();
      }
      setMessage(`Deleted vault entry "${entry.name}".`);
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to delete vault entry",
      );
    } finally {
      setActiveDeleteId(null);
    }
  }

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Vault</p>
          <strong>Stronghold-backed secrets</strong>
        </div>
        <Button
          variant="outline"
          size="sm"
          type="button"
          onClick={() => {
            resetForm();
            setMessage(null);
            setError(null);
          }}
        >
          New entry
        </Button>
      </div>

      <p className="stack-form__note">
        Secret values are deposited from this trusted settings surface and stored
        in the backend vault snapshot. The UI never reads them back after save.
      </p>

      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      {snapshot ? (
        <div className="settings-path-list mb-4">
          <div className="settings-path-row">
            <span>Vault root</span>
            <code>{snapshot.vaultRoot}</code>
          </div>
          <div className="settings-path-row">
            <span>Snapshot path</span>
            <code>{snapshot.snapshotPath}</code>
          </div>
        </div>
      ) : null}

      <form
        className="stack-form settings-profile-form"
        onSubmit={submitVaultEntry}
      >
        <div className="field-grid">
          <label className="field">
            <span>Name</span>
            <Input
              value={form.name}
              onChange={(event) =>
                setForm((current) => ({ ...current, name: event.target.value }))
              }
              placeholder="GitHub Token"
              className="hud-input"
              required
            />
          </label>

          <label className="field">
            <span>Kind</span>
            <select
              value={form.kind}
              onChange={(event) =>
                setForm((current) => ({ ...current, kind: event.target.value }))
              }
            >
              <option value="token">Token</option>
              <option value="password">Password</option>
              <option value="json">JSON blob</option>
              <option value="pem">PEM / key</option>
              <option value="generic">Generic</option>
            </select>
          </label>

          <label className="field">
            <span>Scope tags</span>
            <Input
              value={form.scopeTags}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  scopeTags: event.target.value,
                }))
              }
              placeholder="gh:repo, github:read"
              className="hud-input"
            />
          </label>

          <label className="field">
            <span>Gate policy</span>
            <select
              value={form.gatePolicy}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  gatePolicy: event.target.value,
                }))
              }
            >
              <option value="confirm_session">Confirm per session</option>
              <option value="auto">Auto approve</option>
              <option value="confirm_each_use">Confirm each use</option>
            </select>
          </label>
        </div>

        <label className="field">
          <span>Description</span>
          <Input
            value={form.description}
            onChange={(event) =>
              setForm((current) => ({
                ...current,
                description: event.target.value,
              }))
            }
            placeholder="Used for repository and issue operations."
            className="hud-input"
          />
        </label>

        <label className="field">
          <span>{form.id === null ? "Secret value" : "Rotate value (optional)"}</span>
          <textarea
            rows={6}
            value={form.value}
            onChange={(event) =>
              setForm((current) => ({ ...current, value: event.target.value }))
            }
            placeholder={
              form.id === null
                ? "Paste the secret value once."
                : "Leave blank to keep the current value, or paste a new one to rotate."
            }
            className="hud-input min-h-[8rem] font-mono"
          />
        </label>

        <div className="action-row">
          <Button variant="default" disabled={isSaving} type="submit">
            {isSaving
              ? "Saving..."
              : form.id === null
                ? "Save vault entry"
                : "Update vault entry"}
          </Button>
          {form.id !== null ? (
            <Button variant="outline" type="button" onClick={() => resetForm()}>
              Cancel edit
            </Button>
          ) : null}
        </div>
      </form>

      {isLoading ? (
        <PanelLoadingState
          className="min-h-[18rem]"
          detail="Loading vault metadata from the local Stronghold snapshot."
          eyebrow="Vault"
          title="Opening secret catalog"
          tone="cyan"
        />
      ) : snapshot?.entries.length ? (
        <div className="settings-profile-list">
          {snapshot.entries.map((entry) => (
            <article key={entry.id} className="settings-profile-card">
              <div className="settings-profile-card__header">
                <div className="settings-profile-card__title">
                  <strong>{entry.name}</strong>
                  <div className="settings-profile-card__badges">
                    <Badge className="rounded-full border border-border px-1.5 py-0.5">
                      {entry.kind}
                    </Badge>
                    <Badge className="rounded-full border border-border px-1.5 py-0.5">
                      {formatGatePolicyLabel(entry.gatePolicy)}
                    </Badge>
                  </div>
                </div>
              </div>

              {entry.description ? (
                <p className="stack-form__note">{entry.description}</p>
              ) : null}

              <div className="overview-inline-meta">
                <code>
                  {entry.scopeTags.length > 0
                    ? entry.scopeTags.join(", ")
                    : "No scope tags yet"}
                </code>
                <code>Updated {entry.updatedAt}</code>
              </div>

              <div className="action-row">
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startEdit(entry)}
                >
                  Edit metadata
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startEdit(entry)}
                >
                  Rotate value
                </Button>
                <Button
                  variant="destructive"
                  disabled={activeDeleteId === entry.id}
                  type="button"
                  onClick={() => void deleteEntry(entry)}
                >
                  {activeDeleteId === entry.id ? "Deleting..." : "Delete"}
                </Button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <PanelEmptyState
          className="min-h-[18rem]"
          detail="Create the first vault entry here before wiring integrations or workflow-stage secret requirements."
          eyebrow="Vault"
          title="No secrets stored yet"
          tone="cyan"
        />
      )}
    </article>
  );
}

export default VaultSettingsTab;
