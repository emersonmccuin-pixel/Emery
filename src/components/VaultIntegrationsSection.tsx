import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { invoke } from "@/lib/tauri";
import type {
  VaultEntryRecord,
  VaultIntegrationBindingRecord,
  VaultIntegrationInstallationRecord,
  VaultIntegrationSnapshot,
  VaultIntegrationTemplateRecord,
} from "@/types";

const EMPTY_INTEGRATION_FORM = {
  id: null as number | null,
  templateSlug: "github-rest",
  label: "",
  enabled: true,
  bindings: {} as Record<string, string>,
};

function buildIntegrationForm(
  template: VaultIntegrationTemplateRecord | null,
  installation?: VaultIntegrationInstallationRecord | null,
) {
  const bindings: Record<string, string> = {};
  if (template) {
    for (const slot of template.secretSlots) {
      bindings[slot.slotName] = "";
    }
  }
  for (const binding of installation?.bindings ?? []) {
    bindings[binding.slotName] = binding.entryName;
  }

  return {
    id: installation?.id ?? null,
    templateSlug: installation?.templateSlug ?? template?.slug ?? "github-rest",
    label: installation?.label ?? template?.name ?? "",
    enabled: installation?.enabled ?? true,
    bindings,
  };
}

function integrationDiagnosticsArgs(
  form: typeof EMPTY_INTEGRATION_FORM,
  template: VaultIntegrationTemplateRecord | null,
) {
  const bindings: VaultIntegrationBindingRecord[] = (template?.secretSlots ?? [])
    .map((slot) => ({
      slotName: slot.slotName,
      entryName: form.bindings[slot.slotName] ?? "",
    }))
    .filter((binding) => binding.entryName.trim().length > 0);

  return {
    input: {
      id: form.id,
      templateSlug: form.templateSlug,
      label: form.label,
      enabled: form.enabled,
      bindings,
    },
  };
}

function formatIntegrationKindLabel(kind: string) {
  switch (kind) {
    case "http_broker":
      return "HTTP broker";
    case "cli":
      return "CLI";
    case "mcp":
      return "MCP";
    default:
      return kind;
  }
}

function formatIntegrationPlacementLabel(
  template: VaultIntegrationTemplateRecord,
) {
  const placements = new Set(
    template.secretSlots.map((slot) =>
      slot.placement === "authorization_bearer"
        ? "Bearer header"
        : slot.placement === "env_var"
          ? slot.envVar
            ? `Env ${slot.envVar}`
            : "Environment variable"
        : slot.headerName
          ? `Header ${slot.headerName}`
          : "Header",
    ),
  );
  return Array.from(placements).join(", ");
}

function VaultIntegrationsSection({
  entries,
}: {
  entries: VaultEntryRecord[];
}) {
  const [snapshot, setSnapshot] = useState<VaultIntegrationSnapshot | null>(null);
  const [form, setForm] = useState(EMPTY_INTEGRATION_FORM);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [activeDeleteId, setActiveDeleteId] = useState<number | null>(null);

  useEffect(() => {
    void loadIntegrations();
  }, []);

  const selectedTemplate =
    snapshot?.templates.find((template) => template.slug === form.templateSlug) ??
    snapshot?.templates[0] ??
    null;

  async function loadIntegrations() {
    setIsLoading(true);
    setError(null);

    try {
      const next = await invoke<VaultIntegrationSnapshot>("list_vault_integrations");
      setSnapshot(next);
      const defaultTemplate =
        next.templates.find((template) => template.slug === form.templateSlug) ??
        next.templates[0] ??
        null;
      setForm((current) =>
        current.id === null ? buildIntegrationForm(defaultTemplate) : current,
      );
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to load vault integrations",
      );
    } finally {
      setIsLoading(false);
    }
  }

  function startNew(template?: VaultIntegrationTemplateRecord) {
    setForm(buildIntegrationForm(template ?? selectedTemplate));
    setError(null);
    setMessage(null);
  }

  function startEdit(installation: VaultIntegrationInstallationRecord) {
    setForm(buildIntegrationForm(installation.template ?? null, installation));
    setError(null);
    setMessage(null);
  }

  function updateTemplateSlug(templateSlug: string) {
    const template =
      snapshot?.templates.find((candidate) => candidate.slug === templateSlug) ??
      null;
    setForm((current) => {
      const next = buildIntegrationForm(template);
      next.id = current.id;
      next.label = current.id === null ? template?.name ?? "" : current.label;
      next.enabled = current.enabled;
      for (const [slotName, entryName] of Object.entries(current.bindings)) {
        if (slotName in next.bindings) {
          next.bindings[slotName] = entryName;
        }
      }
      return next;
    });
  }

  async function submitIntegration(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedTemplate) {
      return;
    }

    setIsSaving(true);
    setError(null);
    setMessage(null);

    const bindings = selectedTemplate.secretSlots
      .map((slot) => ({
        slotName: slot.slotName,
        entryName: form.bindings[slot.slotName] ?? "",
      }))
      .filter((binding) => binding.entryName.trim().length > 0);

    try {
      const next = await invoke<VaultIntegrationSnapshot>(
        "upsert_vault_integration",
        {
          input: {
            id: form.id,
            templateSlug: form.templateSlug,
            label: form.label,
            enabled: form.enabled,
            bindings,
          },
        },
        {
          diagnosticsArgs: integrationDiagnosticsArgs(form, selectedTemplate),
        },
      );

      setSnapshot(next);
      setMessage(
        form.id === null
          ? "Brokered integration saved."
          : "Brokered integration updated.",
      );
      startNew(
        next.templates.find((template) => template.slug === form.templateSlug) ??
          next.templates[0],
      );
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to save vault integration",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function deleteIntegration(
    installation: VaultIntegrationInstallationRecord,
  ) {
    if (!confirm(`Delete brokered integration "${installation.label}"?`)) {
      return;
    }

    setActiveDeleteId(installation.id);
    setError(null);
    setMessage(null);

    try {
      const next = await invoke<VaultIntegrationSnapshot>(
        "delete_vault_integration",
        {
          input: { id: installation.id },
        },
      );
      setSnapshot(next);
      if (form.id === installation.id) {
        startNew(next.templates[0]);
      }
      setMessage(`Deleted brokered integration "${installation.label}".`);
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to delete vault integration",
      );
    } finally {
      setActiveDeleteId(null);
    }
  }

  return (
    <section className="settings-section-stack">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Integrations</p>
          <strong>Integration templates</strong>
        </div>
        <Button variant="outline" size="sm" type="button" onClick={() => startNew()}>
          New integration
        </Button>
      </div>

      <p className="stack-form__note">
        These integrations keep auth inside the supervisor. HTTP templates run
        as allowlisted brokered requests, and CLI templates run as
        supervisor-owned child processes with vault-backed env injection.
      </p>

      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      {isLoading ? (
        <PanelLoadingState
          className="min-h-[14rem]"
          detail="Loading the built-in integration template catalog and local bindings."
          eyebrow="Integrations"
          title="Opening brokered templates"
          tone="cyan"
        />
      ) : snapshot ? (
        <>
          <div className="settings-profile-list settings-profile-list--compact">
            {snapshot.templates.map((template) => {
              const installCount = snapshot.installations.filter(
                (installation) => installation.templateSlug === template.slug,
              ).length;

              return (
                <article key={template.slug} className="settings-profile-card">
                  <div className="settings-profile-card__header">
                    <div className="settings-profile-card__title">
                      <strong>{template.name}</strong>
                      <div className="settings-profile-card__badges">
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          {formatIntegrationKindLabel(template.kind)}
                        </Badge>
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          {installCount} configured
                        </Badge>
                      </div>
                    </div>
                  </div>

                  <p className="stack-form__note">{template.description}</p>

                  <div className="overview-inline-meta">
                    <code>{template.baseUrl ?? "No base URL"}</code>
                    <code>{template.egressDomains.join(", ")}</code>
                  </div>

                  <div className="overview-inline-meta">
                    <code>
                      Slots:{" "}
                      {template.secretSlots.map((slot) => slot.slotName).join(", ")}
                    </code>
                    <code>{formatIntegrationPlacementLabel(template)}</code>
                  </div>

                  <div className="action-row">
                    <Button
                      variant="outline"
                      type="button"
                      onClick={() => startNew(template)}
                    >
                      Configure
                    </Button>
                  </div>
                </article>
              );
            })}
          </div>

          {selectedTemplate ? (
            <form
              className="stack-form settings-profile-form mt-6"
              onSubmit={submitIntegration}
            >
              <div className="stack-form__header">
                <h3>
                  {form.id === null
                    ? "Configure brokered integration"
                    : "Edit brokered integration"}
                </h3>
              </div>

              <div className="field-grid">
                <label className="field">
                  <span>Template</span>
                  <select
                    value={form.templateSlug}
                    onChange={(event) => updateTemplateSlug(event.target.value)}
                  >
                    {snapshot.templates.map((template) => (
                      <option key={template.slug} value={template.slug}>
                        {template.name}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>Label</span>
                  <Input
                    value={form.label}
                    onChange={(event) =>
                      setForm((current) => ({
                        ...current,
                        label: event.target.value,
                      }))
                    }
                    placeholder={selectedTemplate.name}
                    className="hud-input"
                    required
                  />
                </label>
              </div>

              <label className="settings-toggle">
                <div>
                  <span>Enabled</span>
                  <p className="stack-form__note">
                    Disabled integrations stay configured but reject brokered
                    requests until re-enabled.
                  </p>
                </div>
                <input
                  checked={form.enabled}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      enabled: event.target.checked,
                    }))
                  }
                  type="checkbox"
                />
              </label>

              <div className="settings-integration-slot-list">
                {selectedTemplate.secretSlots.map((slot) => (
                  <label key={slot.slotName} className="field">
                    <span>
                      {slot.label}
                      <code> {slot.slotName}</code>
                    </span>
                    <select
                      value={form.bindings[slot.slotName] ?? ""}
                      onChange={(event) =>
                        setForm((current) => ({
                          ...current,
                          bindings: {
                            ...current.bindings,
                            [slot.slotName]: event.target.value,
                          },
                        }))
                      }
                    >
                      <option value="">Leave unbound</option>
                      {entries.map((entry) => (
                        <option key={entry.id} value={entry.name}>
                          {entry.name}
                        </option>
                      ))}
                    </select>
                    <p className="stack-form__note">
                      {slot.description} Required scopes:{" "}
                      <code>{slot.requiredScopeTags.join(", ")}</code>
                    </p>
                  </label>
                ))}
              </div>

              <div className="overview-inline-meta">
                <code>{selectedTemplate.baseUrl ?? "No base URL"}</code>
                <code>
                  Allowlisted egress: {selectedTemplate.egressDomains.join(", ")}
                </code>
              </div>

              <div className="action-row">
                <Button variant="default" disabled={isSaving} type="submit">
                  {isSaving
                    ? "Saving..."
                    : form.id === null
                      ? "Save integration"
                      : "Update integration"}
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startNew(selectedTemplate)}
                >
                  Reset form
                </Button>
              </div>
            </form>
          ) : null}

          {snapshot.installations.length > 0 ? (
            <div className="settings-profile-list mt-6">
              {snapshot.installations.map((installation) => (
                <article key={installation.id} className="settings-profile-card">
                  <div className="settings-profile-card__header">
                    <div className="settings-profile-card__title">
                      <strong>{installation.label}</strong>
                      <div className="settings-profile-card__badges">
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          {installation.template?.name ?? installation.templateSlug}
                        </Badge>
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          {installation.enabled ? "Enabled" : "Disabled"}
                        </Badge>
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          {installation.ready ? "Ready" : "Missing bindings"}
                        </Badge>
                      </div>
                    </div>
                  </div>

                  <div className="overview-inline-meta">
                    <code>
                      {installation.bindings.length > 0
                        ? installation.bindings
                            .map(
                              (binding) =>
                                `${binding.slotName}: ${binding.entryName}`,
                            )
                            .join(" • ")
                        : "No secret slots bound yet"}
                    </code>
                    <code>Updated {installation.updatedAt}</code>
                  </div>

                  {installation.missingBindings.length > 0 ? (
                    <p className="stack-form__note">
                      Missing bindings:{" "}
                      <code>{installation.missingBindings.join(", ")}</code>
                    </p>
                  ) : null}

                  <div className="action-row">
                    <Button
                      variant="outline"
                      type="button"
                      onClick={() => startEdit(installation)}
                    >
                      Edit
                    </Button>
                    <Button
                      variant="destructive"
                      disabled={activeDeleteId === installation.id}
                      type="button"
                      onClick={() => void deleteIntegration(installation)}
                    >
                      {activeDeleteId === installation.id
                        ? "Deleting..."
                        : "Delete"}
                    </Button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <PanelEmptyState
              className="min-h-[12rem] mt-6"
              detail="Configure a built-in brokered integration here when you want agents to call an external HTTP API without receiving the raw secret."
              eyebrow="Integrations"
              title="No brokered integrations configured yet"
              tone="cyan"
            />
          )}
        </>
      ) : null}
    </section>
  );
}

export default VaultIntegrationsSection;
