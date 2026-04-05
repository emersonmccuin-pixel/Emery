import { useEffect, useState } from "react";

export function DurationDisplay({ startedAt }: { startedAt: number | null }) {
  const [, setTick] = useState(0);

  useEffect(() => {
    if (!startedAt) return;
    const timer = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(timer);
  }, [startedAt]);

  if (!startedAt) return null;

  const seconds = Math.floor(Date.now() / 1000 - startedAt);
  if (seconds < 60) return <span className="duration">{seconds}s</span>;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return <span className="duration">{minutes}m</span>;
  const hours = Math.floor(minutes / 60);
  return <span className="duration">{hours}h {minutes % 60}m</span>;
}
