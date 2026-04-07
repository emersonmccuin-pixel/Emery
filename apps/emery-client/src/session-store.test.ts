import { describe, expect, it } from "vitest";
import { deriveDisplayState, type SessionSnapshot } from "./session-store";

function snapshot(overrides: Partial<SessionSnapshot> = {}): SessionSnapshot {
  return {
    runtime_state: "running",
    status: "active",
    activity_state: "idle",
    needs_input_reason: null,
    tab_status: null,
    live: true,
    title: "Session",
    current_mode: "execution",
    agent_kind: "claude",
    cwd: "C:\\repo",
    attached_clients: 1,
    ...overrides,
  };
}

describe("deriveDisplayState", () => {
  it("maps waiting_for_input activity to waiting_for_input display", () => {
    expect(
      deriveDisplayState(
        snapshot({
          activity_state: "waiting_for_input",
          needs_input_reason: "approval_required",
        }),
      ),
    ).toBe("waiting_for_input");
  });

  it("prefers tab_status when present", () => {
    expect(
      deriveDisplayState(
        snapshot({
          activity_state: "working",
          tab_status: "waiting",
        }),
      ),
    ).toBe("waiting_for_input");
  });

  it("treats ended sessions as ended even with stale activity state", () => {
    expect(
      deriveDisplayState(
        snapshot({
          live: false,
          activity_state: "waiting_for_input",
        }),
      ),
    ).toBe("ended");
  });

  it("treats failed sessions as errors", () => {
    expect(
      deriveDisplayState(
        snapshot({
          runtime_state: "failed",
        }),
      ),
    ).toBe("error");
  });
});
