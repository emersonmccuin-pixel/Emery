import { useMemo } from "react";
import type { DocumentSummary, WorkItemSummary } from "../types";

type DocsSectionProps = {
  documents: DocumentSummary[];
  workItems: WorkItemSummary[];
  onOpen?: (documentId: string) => void;
};

function formatUpdatedAt(updatedAt: number): string {
  return new Date(updatedAt * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function DocsSection({ documents, workItems, onOpen }: DocsSectionProps) {
  const workItemById = useMemo(() => {
    const map: Record<string, WorkItemSummary> = {};
    for (const item of workItems) map[item.id] = item;
    return map;
  }, [workItems]);

  const grouped = useMemo(() => {
    const types = new Set(documents.map((d) => d.doc_type));
    if (types.size <= 1) return null;
    const groups: Record<string, DocumentSummary[]> = {};
    for (const doc of documents) {
      if (!groups[doc.doc_type]) groups[doc.doc_type] = [];
      groups[doc.doc_type].push(doc);
    }
    return groups;
  }, [documents]);

  if (documents.length === 0) {
    return (
      <section className="project-section docs-section">
        <div className="section-header">
          <h3>Documents</h3>
        </div>
        <p className="section-empty">No documents yet.</p>
      </section>
    );
  }

  return (
    <section className="project-section docs-section">
      <div className="section-header">
        <h3>Documents</h3>
        <span className="section-count">{documents.length}</span>
      </div>

      {grouped ? (
        (Object.entries(grouped) as [string, DocumentSummary[]][]).map(([docType, docs]) => (
          <div key={docType} className="doc-group">
            <div className="doc-group-header">
              <span className="doc-group-label">{docType}</span>
              <span className="doc-group-count">{docs.length}</span>
            </div>
            {docs.map((doc) => (
              <DocRow
                key={doc.id}
                doc={doc}
                linkedCallsign={doc.work_item_id ? (workItemById[doc.work_item_id]?.callsign ?? null) : null}
                onOpen={onOpen}
              />
            ))}
          </div>
        ))
      ) : (
        documents.map((doc) => (
          <DocRow
            key={doc.id}
            doc={doc}
            linkedCallsign={doc.work_item_id ? (workItemById[doc.work_item_id]?.callsign ?? null) : null}
            onOpen={onOpen}
          />
        ))
      )}
    </section>
  );
}

function DocRow({
  doc,
  linkedCallsign,
  onOpen,
}: {
  doc: DocumentSummary;
  linkedCallsign: string | null;
  onOpen?: (documentId: string) => void;
}) {
  return (
    <div className="doc-row" onClick={() => onOpen?.(doc.id)} role={onOpen ? "button" : undefined}>
      <span className="doc-title">{doc.title}</span>
      <span className="doc-type-chip">{doc.doc_type}</span>
      {linkedCallsign ? <span className="doc-linked-callsign">{linkedCallsign}</span> : null}
      <span className="doc-updated">{formatUpdatedAt(doc.updated_at)}</span>
    </div>
  );
}
