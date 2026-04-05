import { useEffect, useRef, useState } from "react";
import type { PlanningAssignmentSummary } from "../types";

function tomorrowCadenceKey(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  return d.toISOString().slice(0, 10);
}

function nextWeekCadenceKey(): string {
  const next = new Date();
  next.setDate(next.getDate() + 7);
  const date = new Date(Date.UTC(next.getUTCFullYear(), next.getUTCMonth(), next.getUTCDate()));
  const day = date.getUTCDay() || 7;
  date.setUTCDate(date.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
  const weekNumber = Math.ceil((((date.getTime() - yearStart.getTime()) / 86400000) + 1) / 7);
  return `${date.getUTCFullYear()}-W${String(weekNumber).padStart(2, "0")}`;
}

type PlanPickerProps = {
  assignments: PlanningAssignmentSummary[];
  dayCadenceKey: string;
  weekCadenceKey: string;
  onToggle: (cadenceType: "day" | "week", cadenceKey: string) => void;
};

export function PlanPicker({ assignments, dayCadenceKey, weekCadenceKey, onToggle }: PlanPickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const activeAssignments = assignments.filter((a) => a.removed_at === null);
  const isAssigned = activeAssignments.length > 0;

  const tomorrow = tomorrowCadenceKey();
  const nextWeek = nextWeekCadenceKey();

  const isAssignedTo = (type: "day" | "week", key: string) =>
    activeAssignments.some((a) => a.cadence_type === type && a.cadence_key === key);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const options: Array<{ label: string; type: "day" | "week"; key: string }> = [
    { label: "Today", type: "day", key: dayCadenceKey },
    { label: "Tomorrow", type: "day", key: tomorrow },
    { label: "This Week", type: "week", key: weekCadenceKey },
    { label: "Next Week", type: "week", key: nextWeek },
  ];

  return (
    <div className="plan-picker" ref={ref}>
      <button
        className={`plan-picker-btn${isAssigned ? " plan-picker-btn-active" : ""}`}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        title="Plan"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <rect x="1" y="2" width="10" height="9" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
          <path d="M1 5h10" stroke="currentColor" strokeWidth="1.2" />
          <path d="M4 1v2M8 1v2" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
      </button>
      {open && (
        <div className="plan-picker-dropdown">
          {options.map(({ label, type, key }) => (
            <button
              key={`${type}:${key}`}
              className={`plan-picker-item${isAssignedTo(type, key) ? " plan-picker-item-checked" : ""}`}
              onClick={(e) => {
                e.stopPropagation();
                onToggle(type, key);
                setOpen(false);
              }}
            >
              <span className="plan-picker-check">{isAssignedTo(type, key) ? "✓" : ""}</span>
              {label}
            </button>
          ))}
          {isAssigned && (
            <>
              <div className="plan-picker-separator" />
              <button
                className="plan-picker-item plan-picker-item-remove"
                onClick={(e) => {
                  e.stopPropagation();
                  for (const a of activeAssignments) {
                    onToggle(a.cadence_type as "day" | "week", a.cadence_key);
                  }
                  setOpen(false);
                }}
              >
                Remove from plan
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
