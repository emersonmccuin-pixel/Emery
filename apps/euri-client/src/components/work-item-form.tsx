import { useState } from "react";
import { WORK_ITEM_TYPES, WORK_ITEM_STATUSES, PRIORITIES } from "../store";

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
        <input
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

      {/* Parent ID */}
      <div className="wi-form-field">
        <label className="wi-form-label">Parent ID</label>
        <input
          className="wi-form-input"
          type="text"
          value={form.parent_id}
          onChange={(e) => set("parent_id", e.target.value)}
          placeholder="Optional parent work item ID"
        />
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
        <button type="submit" className="secondary-button" disabled={!canSubmit}>
          {loading ? "Saving…" : mode === "create" ? "Create" : "Save"}
        </button>
        <button type="button" className="secondary-button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
