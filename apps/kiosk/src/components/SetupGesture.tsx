import { useRef } from 'react';

interface SetupGestureProps {
  /** Number of rapid taps required (default 5). */
  taps?: number;
  /** Window in ms within which the taps must occur (default 3000). */
  windowMs?: number;
  onTrigger: () => void;
}

/**
 * An invisible hotspot in the top-left corner. Rapidly tapping it the required
 * number of times opens setup, keeping the entry point hidden from players
 * (ADR-0020 setup-mode entry).
 */
export function SetupGesture({ taps = 5, windowMs = 3000, onTrigger }: SetupGestureProps) {
  const timestampsRef = useRef<number[]>([]);

  function handleTap() {
    const now = Date.now();
    const recent = [...timestampsRef.current, now].filter((t) => now - t < windowMs);
    timestampsRef.current = recent;
    if (recent.length >= taps) {
      timestampsRef.current = [];
      onTrigger();
    }
  }

  return (
    <button
      type="button"
      aria-hidden="true"
      tabIndex={-1}
      className="setup-gesture"
      onClick={handleTap}
    />
  );
}
