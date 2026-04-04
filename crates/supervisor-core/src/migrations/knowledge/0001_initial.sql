CREATE TABLE work_items (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    parent_id TEXT NULL,
    root_work_item_id TEXT NULL,
    callsign TEXT NOT NULL UNIQUE,
    child_sequence INTEGER NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    acceptance_criteria TEXT NULL,
    work_item_type TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NULL,
    created_by TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    closed_at INTEGER NULL,
    FOREIGN KEY(parent_id) REFERENCES work_items(id),
    FOREIGN KEY(root_work_item_id) REFERENCES work_items(id)
);

CREATE INDEX idx_work_items_project ON work_items(project_id);
CREATE INDEX idx_work_items_parent ON work_items(parent_id);
CREATE INDEX idx_work_items_root ON work_items(root_work_item_id);
CREATE INDEX idx_work_items_status ON work_items(status);
CREATE INDEX idx_work_items_priority ON work_items(priority);

CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    work_item_id TEXT NULL,
    session_id TEXT NULL,
    doc_type TEXT NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    status TEXT NOT NULL,
    content_markdown TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL,
    UNIQUE(project_id, slug),
    FOREIGN KEY(work_item_id) REFERENCES work_items(id)
);

CREATE INDEX idx_documents_project ON documents(project_id);
CREATE INDEX idx_documents_work_item ON documents(work_item_id);
CREATE INDEX idx_documents_doc_type ON documents(doc_type);

CREATE TABLE session_nodes (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL,
    work_item_id TEXT NULL,
    title TEXT NULL,
    summary TEXT NULL,
    agent_kind TEXT NOT NULL,
    completed_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(work_item_id) REFERENCES work_items(id)
);

CREATE INDEX idx_session_nodes_project ON session_nodes(project_id);
CREATE INDEX idx_session_nodes_work_item ON session_nodes(work_item_id);

CREATE TABLE knowledge_entries (
    id TEXT PRIMARY KEY,
    entry_type TEXT NOT NULL,
    title TEXT NULL,
    content TEXT NOT NULL,
    summary TEXT NULL,
    source_session_node_id TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    superseded_by_id TEXT NULL,
    admission_status TEXT NOT NULL,
    FOREIGN KEY(source_session_node_id) REFERENCES session_nodes(id),
    FOREIGN KEY(superseded_by_id) REFERENCES knowledge_entries(id)
);

CREATE INDEX idx_knowledge_entries_type ON knowledge_entries(entry_type);
CREATE INDEX idx_knowledge_entries_source_session ON knowledge_entries(source_session_node_id);
CREATE INDEX idx_knowledge_entries_admission ON knowledge_entries(admission_status);

CREATE TABLE links (
    id TEXT PRIMARY KEY,
    source_entity_type TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_type TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    link_type TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_links_forward ON links(source_entity_type, source_entity_id);
CREATE INDEX idx_links_backward ON links(target_entity_type, target_entity_id);
CREATE INDEX idx_links_type ON links(link_type);

CREATE TABLE embeddings (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    embedding_model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector_blob BLOB NOT NULL,
    input_hash TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(entity_type, entity_id, embedding_model)
);

CREATE TABLE extraction_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    classifier_result TEXT NOT NULL,
    model_ref TEXT NULL,
    status TEXT NOT NULL,
    input_artifact_id TEXT NULL,
    output_artifact_id TEXT NULL,
    confidence REAL NULL,
    failure_reason TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_extraction_runs_session ON extraction_runs(session_id);
CREATE INDEX idx_extraction_runs_status ON extraction_runs(status);

CREATE TABLE admission_decisions (
    id TEXT PRIMARY KEY,
    extraction_run_id TEXT NOT NULL,
    entry_id TEXT NULL,
    decision TEXT NOT NULL,
    reason TEXT NOT NULL,
    review_status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(extraction_run_id) REFERENCES extraction_runs(id),
    FOREIGN KEY(entry_id) REFERENCES knowledge_entries(id)
);

CREATE INDEX idx_admission_decisions_run ON admission_decisions(extraction_run_id);
CREATE INDEX idx_admission_decisions_decision ON admission_decisions(decision);

CREATE VIRTUAL TABLE work_items_fts USING fts5(
    title,
    description,
    acceptance_criteria,
    content='work_items',
    content_rowid='rowid'
);

CREATE VIRTUAL TABLE documents_fts USING fts5(
    title,
    content_markdown,
    content='documents',
    content_rowid='rowid'
);

CREATE VIRTUAL TABLE knowledge_entries_fts USING fts5(
    title,
    content,
    summary,
    content='knowledge_entries',
    content_rowid='rowid'
);
