import { useEffect, useRef, useState, useCallback } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function applyInline(s: string): string {
  return s
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

function renderMarkdown(md: string): string {
  const lines = md.split("\n");
  const out: string[] = [];
  let inCodeBlock = false;
  const codeLines: string[] = [];

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (inCodeBlock) {
        out.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
        codeLines.length = 0;
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      continue;
    }
    if (inCodeBlock) {
      codeLines.push(line);
      continue;
    }

    const escaped = escapeHtml(line);
    if (escaped.startsWith("### ")) {
      out.push(`<h3>${applyInline(escaped.slice(4))}</h3>`);
    } else if (escaped.startsWith("## ")) {
      out.push(`<h2>${applyInline(escaped.slice(3))}</h2>`);
    } else if (escaped.startsWith("# ")) {
      out.push(`<h1>${applyInline(escaped.slice(2))}</h1>`);
    } else if (/^[-*] /.test(escaped)) {
      out.push(`<li>${applyInline(escaped.slice(2))}</li>`);
    } else if (/^\d+\. /.test(escaped)) {
      out.push(`<li>${applyInline(escaped.replace(/^\d+\. /, ""))}</li>`);
    } else if (escaped === "") {
      out.push('<div class="md-spacer"></div>');
    } else {
      out.push(`<p>${applyInline(escaped)}</p>`);
    }
  }

  // Close any unclosed code block
  if (inCodeBlock && codeLines.length > 0) {
    out.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
  }

  return out.join("\n");
}

export function DocumentView({
  documentId,
  projectId,
}: {
  documentId: string;
  projectId: string;
}) {
  const doc = useAppStore((s) => s.documentDetails[documentId] ?? null);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const bootstrap = useAppStore((s) => s.bootstrap);

  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [content, setContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");

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
      content_markdown: content,
    });
    setSavedContent(content);
    setSaveStatus("saved");
    setTimeout(() => setSaveStatus("idle"), 2000);
  }, [doc, documentId, content, isDirty, isSaving]);

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
        {doc.work_item_id && (
          <span className="doc-linked-callsign doc-view-linked">linked</span>
        )}
      </div>

      <div className="document-view-toolbar">
        <div className="document-mode-toggle">
          <button
            className={`mode-btn${mode === "edit" ? " mode-btn--active" : ""}`}
            onClick={() => setMode("edit")}
          >
            Edit
          </button>
          <button
            className={`mode-btn${mode === "preview" ? " mode-btn--active" : ""}`}
            onClick={() => setMode("preview")}
          >
            Preview
          </button>
        </div>
        <div className="document-view-actions">
          {saveStatus === "saving" && (
            <span className="doc-save-status">Saving…</span>
          )}
          {saveStatus === "saved" && (
            <span className="doc-save-status doc-save-status--done">Saved</span>
          )}
          <button
            className="doc-save-btn"
            onClick={() => void handleSave()}
            disabled={!isDirty || isSaving}
          >
            Save
          </button>
        </div>
      </div>

      <div className="document-view-body">
        {mode === "edit" ? (
          <textarea
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
        <button
          className="breadcrumb-link"
          onClick={() => navStore.goToProject(projectId)}
        >
          ← {project?.name ?? "Project"}
        </button>
      </div>
    </div>
  );
}
