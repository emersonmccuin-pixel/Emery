import { afterEach, describe, expect, it } from "vitest";
import { getStoredValue, removeStoredValue, setStoredValue } from "./local-storage";

afterEach(() => {
  localStorage.clear();
});

describe("local storage migration helpers", () => {
  it("prefers the current key when both current and legacy values exist", () => {
    localStorage.setItem("emery.theme", "default");
    localStorage.setItem("euri.theme", "legacy");

    expect(getStoredValue("emery.theme", "euri.theme")).toBe("default");
    expect(localStorage.getItem("emery.theme")).toBe("default");
    expect(localStorage.getItem("euri.theme")).toBe("legacy");
  });

  it("migrates a legacy value forward on first read", () => {
    localStorage.setItem("euri.theme", "noir");

    expect(getStoredValue("emery.theme", "euri.theme")).toBe("noir");
    expect(localStorage.getItem("emery.theme")).toBe("noir");
    expect(localStorage.getItem("euri.theme")).toBeNull();
  });

  it("writes the current key and clears the legacy key", () => {
    localStorage.setItem("euri.github_token", "old-token");

    setStoredValue("emery.github_token", "new-token", "euri.github_token");

    expect(localStorage.getItem("emery.github_token")).toBe("new-token");
    expect(localStorage.getItem("euri.github_token")).toBeNull();
  });

  it("removes both current and legacy keys together", () => {
    localStorage.setItem("emery:scratch-project-id", "proj_current");
    localStorage.setItem("euri:scratch-project-id", "proj_legacy");

    removeStoredValue("emery:scratch-project-id", "euri:scratch-project-id");

    expect(localStorage.getItem("emery:scratch-project-id")).toBeNull();
    expect(localStorage.getItem("euri:scratch-project-id")).toBeNull();
  });
});
