import { useEffect, useRef, useState, useCallback } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { Button, Input, Select, Textarea } from "../components/ui";
import { renderMarkdown } from "../utils/markdown";

const DOC_TYPE_SUGGESTIONS = ["note", "prd", "architecture", "gameplan", "meeting", "adr", "runbook"];
const DOC_STATUS_OPTIONS = ["draft", "active", "archived"];

// ── Creation mode ────────────────────────────────────────────────────────────

function NewDocumentView({
  projectId,
  initialWorkItemId,
}: {
  projectId: string;
  initialWorkItemId?: string;
}) {
  const isCreating = useAppStore((s) => s.loadingKeys["create-document"] ?? false);
  const workItems = useAppStore((s) => s.workItemsByProject[projectId] ?? []);

  const [title, setTitle] = useState("");
  const [docType, setDocType] = useState("note");
  const [status, setStatus] = useState("draft");
  const [workItemId, setWorkItemId] = useState(initialWorkItemId ?? "");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);

  const project = useAppStore((s) => s.bootstrap?.projects.find((p) => p.id === projectId) ?? null);

  const handleCreate = useCallback(async () => {
    if (!title.trim()) {
      setError("Title is required.");
      return;
    }
    const detail = await appStore.handleCreateDocumentWithParams({
      project_id: projectId,
      title: title.trim(),
      doc_type: docType.trim() || "note",
      status,
      content_markdown: content,
      work_item_id: workItemId || null,
    });
    if (detail) {
      navStore.goToDocument(projectId, detail.id);
    }
  }, [projectId, title, docType, status, content, workItemId]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        void handleCreate();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handleCreate]);

  return (
    <div className="document-view">
      <div className="document-create-form">
        <h2 className="document-create-title">New Document</h2>

        <div className="doc-meta-panel">
          <div className="doc-meta-row">
            <label className="doc-meta-label">Title</label>
            <Input
              className="doc-meta-input doc-meta-input--wide"
              type="text"
              value={title}
              onChange={(e) => { setTitle(e.target.value); setError(null); }}
              placeholder="Document title"
              autoFocus
            />
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Type</label>
            <Input
              className="doc-meta-input"
              type="text"
              list="doc-type-options"
              value={docType}
              onChange={(e) => setDocType(e.target.value)}
              placeholder="note"
            />
            <datalist id="doc-type-options">
              {DOC_TYPE_SUGGESTIONS.map((t) => <option key={t} value={t} />)}
            </datalist>
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Status</label>
            <Select
              className="doc-meta-select"
              value={status}
              onChange={(e) => setStatus(e.target.value)}
            >
              {DOC_STATUS_OPTIONS.map((s) => (
                <option key={s} value={s}>{s}</option>
              ))}
            </Select>
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Work item</label>
            <Select
              className="doc-meta-select"
              value={workItemId}
              onChange={(e) => setWorkItemId(e.target.value)}
            >
              <option value="">— none —</option>
              {workItems.map((w) => (
                <option key={w.id} value={w.id}>{w.callsign} {w.title}</option>
              ))}
            </Select>
          </div>
        </div>

        <Textarea
          className="document-editor document-editor--create"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Content (optional)"
          spellCheck={false}
        />

        {error && <p className="doc-create-error">{error}</p>}

        <div className="document-create-actions">
          <Button
            onClick={() => void handleCreate()}
            disabled={isCreating}
          >
            {isCreating ? "Creating…" : "Create"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => navStore.goToProject(projectId)}
          >
            Cancel
          </Button>
        </div>
      </div>

      <div className="document-view-footer">
        <Button
          className="breadcrumb-link"
          variant="ghost"
          size="sm"
          onClick={() => navStore.goToProject(projectId)}
        >
          ← {project?.name ?? "Project"}
        </Button>
      </div>
    </div>
  );
}

// ── Existing document mode ───────────────────────────────────────────────────

export function DocumentView({
  documentId,
  projectId,
  workItemId: initialWorkItemId,
}: {
  documentId: string;
  projectId: string;
  workItemId?: string;
}) {
  if (documentId === "new") {
    return (
      <div className="modal-view-wide">
        <NewDocumentView projectId={projectId} initialWorkItemId={initialWorkItemId} />
      </div>
    );
  }
  return (
    <div className="modal-view-wide">
      <ExistingDocumentView documentId={documentId} projectId={projectId} />
    </div>
  );
}

function ExistingDocumentView({
  documentId,
  projectId,
}: {
  documentId: string;
  projectId: string;
}) {
  const doc = useAppStore((s) => s.documentDetails[documentId] ?? null);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const workItems = useAppStore((s) => s.workItemsByProject[projectId] ?? []);

  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [content, setContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");

  // Metadata editing state
  const [metaOpen, setMetaOpen] = useState(false);
  const [metaTitle, setMetaTitle] = useState("");
  const [metaDocType, setMetaDocType] = useState("");
  const [metaStatus, setMetaStatus] = useState("");
  const [metaWorkItemId, setMetaWorkItemId] = useState("");
  const [metaSaveStatus, setMetaSaveStatus] = useState<"idle" | "saving" | "saved">("idle");

  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const isLoading = !!loadingKeys[`document:${documentId}`];
  const isSaving = !!loadingKeys[`save-document:${documentId}`];
  const isDirty = content !== savedContent;

  useEffect(() => {
    void appStore.ensureDocumentDetail(documentId);
  }, [documentId]);

  // Sync content only when document identity changes (not on every update)
  useEffect(() => {
    if (doc) {
      setContent(doc.content_markdown);
      setSavedContent(doc.content_markdown);
      setMetaTitle(doc.title);
      setMetaDocType(doc.doc_type);
      setMetaStatus(doc.status);
      setMetaWorkItemId(doc.work_item_id ?? "");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc?.id]);

  const project = bootstrap?.projects.find((p) => p.id === projectId) ?? null;

  const handleSave = useCallback(async () => {
    if (!doc || !isDirty || isSaving) return;
    setSaveStatus("saving");
    await appStore.handleUpdateDocument(documentId, {
      doc_type: doc.doc_type,
      title: doc.title,
      status: doc.status,
      work_item_id: doc.work_item_id,
      content_markdown: content,
    });
    setSavedContent(content);
    setSaveStatus("saved");
    setTimeout(() => setSaveStatus("idle"), 2000);
  }, [doc, documentId, content, isDirty, isSaving]);

  const handleSaveMeta = useCallback(async () => {
    if (!doc) return;
    setMetaSaveStatus("saving");
    await appStore.handleUpdateDocument(documentId, {
      doc_type: metaDocType.trim() || doc.doc_type,
      title: metaTitle.trim() || doc.title,
      status: metaStatus || doc.status,
      work_item_id: metaWorkItemId || null,
      content_markdown: content,
    });
    setMetaSaveStatus("saved");
    setTimeout(() => setMetaSaveStatus("idle"), 2000);
  }, [doc, documentId, metaTitle, metaDocType, metaStatus, metaWorkItemId, content]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        void handleSave();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handleSave]);

  if (isLoading && !doc) {
    return <div className="document-view document-view--loading">Loading document…</div>;
  }

  if (!doc) {
    return <div className="document-view document-view--error">Document not found.</div>;
  }

  const isMetaDirty =
    metaTitle !== doc.title ||
    metaDocType !== doc.doc_type ||
    metaStatus !== doc.status ||
    metaWorkItemId !== (doc.work_item_id ?? "");

  return (
    <div className="document-view">
      <div className="document-view-header">
        <div className="document-view-meta">
          <h2 className="document-view-title">{doc.title}</h2>
          <div className="document-view-chips">
            <span className="doc-type-chip">{doc.doc_type}</span>
            <span className="doc-status-badge">{doc.status}</span>
            {isDirty && <span className="doc-unsaved-dot" title="Unsaved changes" />}
          </div>
        </div>
        <Button
          className={`doc-meta-toggle${metaOpen ? " doc-meta-toggle--active" : ""}`}
          variant="ghost"
          size="sm"
          onClick={() => setMetaOpen((v) => !v)}
          title="Edit metadata"
        >
          ⋯
        </Button>
      </div>

      {metaOpen && (
        <div className="doc-meta-panel doc-meta-panel--overlay">
          <div className="doc-meta-row">
            <label className="doc-meta-label">Title</label>
            <Input
              className="doc-meta-input doc-meta-input--wide"
              type="text"
              value={metaTitle}
              onChange={(e) => setMetaTitle(e.target.value)}
            />
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Type</label>
            <Input
              className="doc-meta-input"
              type="text"
              list="doc-type-options-edit"
              value={metaDocType}
              onChange={(e) => setMetaDocType(e.target.value)}
            />
            <datalist id="doc-type-options-edit">
              {DOC_TYPE_SUGGESTIONS.map((t) => <option key={t} value={t} />)}
            </datalist>
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Status</label>
            <Select
              className="doc-meta-select"
              value={metaStatus}
              onChange={(e) => setMetaStatus(e.target.value)}
            >
              {DOC_STATUS_OPTIONS.map((s) => (
                <option key={s} value={s}>{s}</option>
              ))}
            </Select>
          </div>
          <div className="doc-meta-row">
            <label className="doc-meta-label">Work item</label>
            <Select
              className="doc-meta-select"
              value={metaWorkItemId}
              onChange={(e) => setMetaWorkItemId(e.target.value)}
            >
              <option value="">— none —</option>
              {workItems.map((w) => (
                <option key={w.id} value={w.id}>{w.callsign} {w.title}</option>
              ))}
            </Select>
          </div>
          <div className="doc-meta-actions">
            {metaSaveStatus === "saving" && <span className="doc-save-status">Saving…</span>}
            {metaSaveStatus === "saved" && <span className="doc-save-status doc-save-status--done">Saved</span>}
            <Button
              onClick={() => void handleSaveMeta()}
              disabled={!isMetaDirty || metaSaveStatus === "saving"}
            >
              Save metadata
            </Button>
          </div>
        </div>
      )}

      <div className="document-view-toolbar">
        <div className="document-mode-toggle">
          <Button
            className={`mode-btn${mode === "edit" ? " mode-btn--active" : ""}`}
            variant={mode === "edit" ? "default" : "ghost"}
            size="sm"
            onClick={() => setMode("edit")}
          >
            Edit
          </Button>
          <Button
            className={`mode-btn${mode === "preview" ? " mode-btn--active" : ""}`}
            variant={mode === "preview" ? "default" : "ghost"}
            size="sm"
            onClick={() => setMode("preview")}
          >
            Preview
          </Button>
        </div>
        <div className="document-view-actions">
          {saveStatus === "saving" && (
            <span className="doc-save-status">Saving…</span>
          )}
          {saveStatus === "saved" && (
            <span className="doc-save-status doc-save-status--done">Saved</span>
          )}
          <Button
            onClick={() => void handleSave()}
            disabled={!isDirty || isSaving}
          >
            Save
          </Button>
        </div>
      </div>

      <div className="document-view-body">
        {mode === "edit" ? (
          <Textarea
            ref={textareaRef}
            className="document-editor"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            spellCheck={false}
          />
        ) : (
          <div
            className="document-preview"
            // renderMarkdown escapes all HTML before applying inline transforms
            // eslint-disable-next-line react/no-danger
            dangerouslySetInnerHTML={{ __html: renderMarkdown(content) }}
          />
        )}
      </div>

      <div className="document-view-footer">
        <Button
          className="breadcrumb-link"
          variant="ghost"
          size="sm"
          onClick={() => navStore.goToProject(projectId)}
        >
          ← {project?.name ?? "Project"}
        </Button>
      </div>
    </div>
  );
}
