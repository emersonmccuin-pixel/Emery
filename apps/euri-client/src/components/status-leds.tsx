import type { GitHealthStatus } from "../types";

type LedState = "green" | "red" | "amber" | "off";

type LedDef = {
  key: string;
  label: string;
  state: LedState;
  title: string;
};

function deriveLeds(status: GitHealthStatus | null): LedDef[] {
  if (!status) {
    return [
      { key: "remote", label: "Remote", state: "off", title: "Not a git repository" },
      { key: "backed-up", label: "Backed Up", state: "off", title: "Not a git repository" },
      { key: "clean", label: "Clean", state: "off", title: "Not a git repository" },
      { key: "up-to-date", label: "Up to Date", state: "off", title: "Not a git repository" },
    ];
  }

  // Remote LED: green = has remote, red = no remote
  const remoteLed: LedDef = {
    key: "remote",
    label: "Remote",
    state: status.has_remote ? "green" : "red",
    title: status.has_remote ? "Remote configured" : "No remote configured",
  };

  // Backed Up LED: green = pushed, red = not pushed, amber = unknown (no upstream), off = no remote
  let backedUpState: LedState;
  let backedUpTitle: string;
  if (!status.has_remote) {
    backedUpState = "off";
    backedUpTitle = "No remote configured";
  } else if (status.is_pushed === null) {
    backedUpState = "amber";
    backedUpTitle = "No upstream branch tracked";
  } else if (status.is_pushed) {
    backedUpState = "green";
    backedUpTitle = "All commits pushed to remote";
  } else {
    backedUpState = "red";
    backedUpTitle = "Unpushed local commits";
  }
  const backedUpLed: LedDef = {
    key: "backed-up",
    label: "Backed Up",
    state: backedUpState,
    title: backedUpTitle,
  };

  // Clean LED: green = no uncommitted changes, red = dirty
  const cleanLed: LedDef = {
    key: "clean",
    label: "Clean",
    state: status.is_clean ? "green" : "red",
    title: status.is_clean ? "Working tree is clean" : "Uncommitted changes present",
  };

  // Up to Date LED: green = not behind, red = behind, amber = unknown (no upstream), off = no remote
  let upToDateState: LedState;
  let upToDateTitle: string;
  if (!status.has_remote) {
    upToDateState = "off";
    upToDateTitle = "No remote configured";
  } else if (status.is_behind === null) {
    upToDateState = "amber";
    upToDateTitle = "No upstream branch tracked";
  } else if (status.is_behind) {
    upToDateState = "red";
    upToDateTitle = "Behind remote — pull needed";
  } else {
    upToDateState = "green";
    upToDateTitle = "Up to date with remote";
  }
  const upToDateLed: LedDef = {
    key: "up-to-date",
    label: "Up to Date",
    state: upToDateState,
    title: upToDateTitle,
  };

  return [remoteLed, backedUpLed, cleanLed, upToDateLed];
}

function formatSyncTime(epochSeconds: number | null): string | undefined {
  if (!epochSeconds) return undefined;
  const date = new Date(epochSeconds * 1000);
  return `Last sync: ${date.toLocaleString()}`;
}

export function StatusLEDs({
  status,
  compact = false,
}: {
  status: GitHealthStatus | null;
  compact?: boolean;
}) {
  const leds = deriveLeds(status);
  const syncTitle = status ? formatSyncTime(status.last_sync_at) : undefined;

  return (
    <div
      className={`status-leds${compact ? " status-leds-compact" : ""}`}
      title={syncTitle}
    >
      {leds.map((led) => (
        <div key={led.key} className="status-led-item" title={led.title}>
          <span className={`status-led status-led-${led.state}`} />
          {!compact && (
            <span className="status-led-label">{led.label}</span>
          )}
        </div>
      ))}
    </div>
  );
}
