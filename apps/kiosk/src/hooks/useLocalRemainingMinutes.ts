import { useEffect, useRef, useState } from 'react';

/**
 * Smooth local countdown anchored to server-authoritative remaining minutes.
 * Re-syncs whenever the authoritative value changes (poll / heartbeat).
 */
export function useLocalRemainingMinutes(authoritativeMinutes: number | undefined): number | null {
  const deadlineRef = useRef<number | null>(null);
  const [localMinutes, setLocalMinutes] = useState<number | null>(() =>
    typeof authoritativeMinutes === 'number' ? authoritativeMinutes : null,
  );

  useEffect(() => {
    if (typeof authoritativeMinutes !== 'number') {
      deadlineRef.current = null;
      setLocalMinutes(null);
      return;
    }

    deadlineRef.current = Date.now() + authoritativeMinutes * 60_000;
    setLocalMinutes(authoritativeMinutes);

    const id = setInterval(() => {
      const deadline = deadlineRef.current;
      if (deadline === null) return;
      setLocalMinutes(Math.max(0, (deadline - Date.now()) / 60_000));
    }, 1000);

    return () => clearInterval(id);
  }, [authoritativeMinutes]);

  return localMinutes;
}
