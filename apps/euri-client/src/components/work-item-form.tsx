import { useRef, useState } from "react";
import { WORK_ITEM_TYPES, WORK_ITEM_STATUSES, PRIORITIES, useAppStore } from "../store";
import type { WorkItemSummary } from "../types";
import { Button, Input } from "./ui";

export type WorkItemFormData = {
  title: string;
  description: string;
  acceptance_criteria: string;
  work_item_type: string;
  status: string;
  priority: string;
  parent_id: string;
};

type WorkItemFormProps = {
  mode: "create" | "edit";
  initialData?: Partial<WorkItemFormData>;
  loading?: boolean;
  /** When set, the parent is pre-locked (e.g. "Add Child" context) and the picker is read-only */
  lockedParentLabel?: string;
  onSubmit: (data: WorkItemFormData) => void;
  onCancel: () => void;
};

const DEFAULT_DATA: WorkItemFormData = {
  title: "",
  description: "",
  acceptance_criteria: "",
  work_item_type: "task",
  status: "backlog",
  priority: "",
  parent_id: "",
};

export function WorkItemForm({
  mode,
  initialData,
  loading = false,
  lockedParentLabel,
  onSubmit,
  onCancel,
}: WorkItemFormProps) {
  const [form, setForm] = useState<WorkItemFormData>({
    ...DEFAULT_DATA,
    ...initialData,
  });

  function set(field: keyof WorkItemFormData, value: string) {
    setForm((prev) => ({ ...prev, [field]: value }));
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!form.title.trim()) return;
    onSubmit(form);
  }

  const canSubmit = form.title.trim().length > 0 && !loading;

  return (
    <form className="wi-form" onSubmit={handleSubmit}>
      {/* Title */}
      <div className="wi-form-field">
        <label className="wi-form-label">Title *</label>
        <Input
          className="wi-form-input"
          type="text"
          value={form.title}
          onChange={(e) => set("title", e.target.value)}
          placeholder="Work item title"
          autoFocus
        />
      </div>

      {/* Type + Priority row */}
      <div className="wi-form-row">
        <div className="wi-form-field">
          <label className="wi-form-label">Type</label>
          <div className="wi-form-pills">
            {WORK_ITEM_TYPES.map((t) => (
              <button
                key={t}
                type="button"
                className={`wi-form-pill${form.work_item_type === t ? " wi-form-pill-active" : ""}`}
                onClick={() => set("work_item_type", t)}
              >
                {t}
              </button>
            ))}
          </div>
        </div>

        <div className="wi-form-field">
          <label className="wi-form-label">Priority</label>
          <div className="wi-form-pills">
            {PRIORITIES.filter((p) => p !== "").map((p) => (
              <button
                key={p}
                type="button"
                className={`wi-form-pill${form.priority === p ? " wi-form-pill-active" : ""}`}
                onClick={() => set("priority", form.priority === p ? "" : p)}
              >
                {p}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Status */}
      <div className="wi-form-field">
        <label className="wi-form-label">Status</label>
        <select
          className="wi-form-input"
          value={form.status}
          onChange={(e) => set("status", e.target.value)}
        >
          {WORK_ITEM_STATUSES.map((s) => (
            <option key={s} value={s}>
              {s.replace("_", " ")}
            </option>
          ))}
        </select>
      </div>

      {/* Parent */}
      <div className="wi-form-field">
        <label className="wi-form-label">Parent</label>
        {lockedParentLabel ? (
          <span className="wi-form-parent-locked">{lockedParentLabel}</span>
        ) : (
          <ParentPicker
            value={form.parent_id}
            onChange={(id) => set("parent_id", id)}
          />
        )}
      </div>

      {/* Description */}
      <div className="wi-form-field">
        <label className="wi-form-label">Description</label>
        <textarea
          className="wi-form-textarea"
          value={form.description}
          onChange={(e) => set("description", e.target.value)}
          placeholder="Describe the work item..."
          rows={4}
        />
      </div>

      {/* Acceptance Criteria */}
      <div className="wi-form-field">
        <label className="wi-form-label">Acceptance Criteria</label>
        <textarea
          className="wi-form-textarea"
          value={form.acceptance_criteria}
          onChange={(e) => set("acceptance_criteria", e.target.value)}
          placeholder="What does done look like?"
          rows={3}
        />
      </div>

      {/* Actions */}
      <div className="wi-form-actions">
        <Button type="submit" disabled={!canSubmit}>
          {loading ? "Saving…" : mode === "create" ? "Create" : "Save"}
        </Button>
        <Button type="button" variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}

// --- ParentPicker ---

function ParentPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (id: string) => void;
}) {
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const workItemDetails = useAppStore((s) => s.workItemDetails);

  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const allItems: WorkItemSummary[] = selectedProjectId
    ? (workItemsByProject[selectedProjectId] ?? [])
    : [];

  const selectedItem = value
    ? (workItemDetails[value] ?? allItems.find((i) => i.id === value) ?? null)
    : null;

  const filtered = allItems
    .filter((item) => {
      if (!query.trim()) return true;
      const q = query.toLowerCase();
      return (
        item.callsign.toLowerCase().includes(q) ||
        item.title.toLowerCase().includes(q)
      );
    })
    .slice(0, 10);

  function handleSelect(item: WorkItemSummary) {
    onChange(item.id);
    setQuery("");
    setOpen(false);
  }

  function handleClear() {
    onChange("");
    setQuery("");
    setOpen(false);
  }

  return (
    <div className="parent-picker" ref={containerRef}>
      {selectedItem ? (
        <div className="parent-picker-selected">
          <span className="parent-picker-value">
            {selectedItem.callsign}: {selectedItem.title}
          </span>
          <Button
            type="button"
            className="parent-picker-clear"
            variant="ghost"
            size="sm"
            onClick={handleClear}
            title="Clear parent"
          >
            ✕
          </Button>
        </div>
      ) : (
        <Input
          className="wi-form-input"
          type="text"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => {
            // Delay so click on dropdown option registers first
            setTimeout(() => setOpen(false), 150);
          }}
          placeholder="Search by callsign or title…"
        />
      )}
      {open && !selectedItem && filtered.length > 0 && (
        <div className="parent-picker-dropdown">
          {filtered.map((item) => (
            <Button
              key={item.id}
              type="button"
              className="parent-picker-option"
              variant="ghost"
              onMouseDown={(e) => {
                e.preventDefault(); // prevent blur before click
                handleSelect(item);
              }}
            >
              <span className="parent-picker-callsign">{item.callsign}</span>
              <span className="parent-picker-title">{item.title}</span>
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
