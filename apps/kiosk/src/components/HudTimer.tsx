import { useEffect, useRef, useState } from 'react';

interface HudTimerProps {
  /** Server-authoritative remaining minutes; resets the local countdown. */
  remainingMinutes: number;
}

function formatClock(totalSeconds: number): string {
  const safe = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const seconds = safe % 60;
  const pad = (n: number) => n.toString().padStart(2, '0');
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}

/**
 * Always-visible countdown. Ticks locally every second for smoothness and
 * re-anchors to the server value whenever `remainingMinutes` changes (poll/WS).
 */
export function HudTimer({ remainingMinutes }: HudTimerProps) {
  const deadlineRef = useRef<number>(Date.now() + remainingMinutes * 60_000);
  const [secondsLeft, setSecondsLeft] = useState(() => remainingMinutes * 60);

  // Re-anchor whenever the authoritative value changes.
  useEffect(() => {
    deadlineRef.current = Date.now() + remainingMinutes * 60_000;
    setSecondsLeft(remainingMinutes * 60);
  }, [remainingMinutes]);

  useEffect(() => {
    const id = setInterval(() => {
      setSecondsLeft(Math.max(0, (deadlineRef.current - Date.now()) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, []);

  const minutesLeft = secondsLeft / 60;
  const tone = minutesLeft <= 1 ? 'critical' : minutesLeft <= 5 ? 'warning' : 'normal';

  return (
    <div className={`hud-timer hud-timer-${tone}`} aria-live="polite">
      <span className="hud-timer-label">Time remaining</span>
      <span className="hud-timer-value">{formatClock(secondsLeft)}</span>
    </div>
  );
}
