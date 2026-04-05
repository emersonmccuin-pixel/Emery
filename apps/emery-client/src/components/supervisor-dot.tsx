import { useAppStore } from "../store";

export function SupervisorDot() {
  const connectionState = useAppStore((s) => s.connectionState);
  const isConnected = connectionState === "connected";

  return (
    <span
      className={`supervisor-dot supervisor-dot-${isConnected ? "ok" : "err"}`}
      title={isConnected ? "Supervisor connected" : `Supervisor: ${connectionState}`}
    />
  );
}
