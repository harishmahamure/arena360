import { formatRemainingClock } from '@gaming-cafe/utils';

interface HudTimerProps {
  /** Pre-interpolated remaining wallet minutes (from `useSessionRemainingMinutes`). */
  remainingMinutes: number;
}

/**
 * Always-visible countdown label. Expects wallet minutes already adjusted for
 * dynamic deduction by the parent hook; does not run its own wall-clock math.
 */
export function HudTimer({ remainingMinutes }: HudTimerProps) {
  const minutesLeft = remainingMinutes;
  const tone = minutesLeft <= 1 ? 'critical' : minutesLeft <= 5 ? 'warning' : 'normal';

  return (
    <div className={`hud-timer hud-timer-${tone}`} aria-live="polite">
      <span className="hud-timer-label">Time remaining</span>
      <span className="hud-timer-value">{formatRemainingClock(remainingMinutes)}</span>
    </div>
  );
}
