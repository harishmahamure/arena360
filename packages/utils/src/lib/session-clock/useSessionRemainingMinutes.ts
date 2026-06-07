import type { DeductionProfile } from '@gaming-cafe/contracts';
import { useEffect, useRef, useState } from 'react';
import { interpolateRemainingMinutes } from './interpolateRemainingMinutes.js';

/**
 * Shared session countdown for admin and kiosk.
 *
 * Anchors to server-authoritative wallet minutes and ticks locally between
 * syncs. When a deduction profile is present, burn rate follows the current
 * venue-local ratio (peak faster, low slower). Without a profile, 1 wall
 * minute = 1 wallet minute.
 *
 * @see docs/session-time-clock.md
 */
export function useSessionRemainingMinutes(
  authoritativeMinutes: number | undefined,
  deductionProfile?: DeductionProfile | null,
  cafeTimezone?: string,
): number | null {
  const anchorRef = useRef<{
    remaining: number;
    syncedAt: number;
  } | null>(null);
  const [localMinutes, setLocalMinutes] = useState<number | null>(() =>
    typeof authoritativeMinutes === 'number' ? authoritativeMinutes : null,
  );

  useEffect(() => {
    if (typeof authoritativeMinutes !== 'number') {
      anchorRef.current = null;
      setLocalMinutes(null);
      return;
    }

    anchorRef.current = {
      remaining: authoritativeMinutes,
      syncedAt: Date.now(),
    };
    setLocalMinutes(authoritativeMinutes);

    const id = setInterval(() => {
      const anchor = anchorRef.current;
      if (!anchor) return;
      setLocalMinutes(
        interpolateRemainingMinutes(
          anchor.remaining,
          anchor.syncedAt,
          Date.now(),
          deductionProfile,
          cafeTimezone,
        ),
      );
    }, 1000);

    return () => clearInterval(id);
  }, [authoritativeMinutes, deductionProfile, cafeTimezone]);

  return localMinutes;
}
