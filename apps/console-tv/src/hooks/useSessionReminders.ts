import { useEffect, useRef } from 'react';
import { playReminder } from '../lib/native/ConsoleNative';

const THRESHOLDS = [
  { minutes: 10, key: 'ten' as const },
  { minutes: 5, key: 'five' as const },
  { minutes: 2, key: 'two' as const },
];

/**
 * Play audio reminders when remaining time crosses 10 / 5 / 2 minutes.
 */
export function useSessionReminders(
  remainingMinutes: number | null | undefined,
  active: boolean,
): void {
  const firedRef = useRef<Set<number>>(new Set());

  useEffect(() => {
    if (!active) {
      firedRef.current.clear();
      return;
    }
  }, [active]);

  useEffect(() => {
    if (!active || remainingMinutes == null) return;

    for (const { minutes, key } of THRESHOLDS) {
      if (remainingMinutes <= minutes && !firedRef.current.has(minutes)) {
        firedRef.current.add(minutes);
        playReminder(key);
      }
    }
  }, [active, remainingMinutes]);
}
