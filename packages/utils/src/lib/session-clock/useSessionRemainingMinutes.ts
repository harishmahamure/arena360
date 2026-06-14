import type { DeductionProfile } from '@gaming-cafe/contracts';
import {
  capRemainingByExpiry,
  DEFAULT_CAFE_TZ,
  effectiveRemainingMinutes,
  SESSION_CLOCK_TICK_MS,
  walletBalanceFromEffectiveRemaining,
} from '@gaming-cafe/contracts';
import { useEffect, useRef, useState } from 'react';

export interface SessionRemainingClockInput {
  sessionStartTime?: string;
  /** Raw wallet balance minutes (admin sessions list). */
  walletBalanceMinutes?: number;
  /**
   * Server effective remaining — kiosk re-anchors wallet balance from this on
   * session start and `balance.updated`. Omit when `walletBalanceMinutes` is set.
   */
  serverEffectiveRemainingMinutes?: number;
  timeCreditsConsumed?: number | null;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  /** Plan balance expiry — caps display at min(wallet, minutesUntilExpiry). */
  expiryDate?: string | null;
}

/**
 * Shared session countdown for admin, kiosk, and console TV.
 *
 * Ticks locally from `sessionStartTime` using backend-aligned
 * `weightedMinutesBetween`. Re-anchors wallet balance when server values change.
 *
 * @see docs/session-time-clock.md
 */
export function useSessionRemainingMinutes(
  input: SessionRemainingClockInput | undefined,
): number | null {
  const walletRef = useRef<number | null>(null);
  const [localMinutes, setLocalMinutes] = useState<number | null>(null);

  const sessionStartTime = input?.sessionStartTime;
  const walletBalanceMinutes = input?.walletBalanceMinutes;
  const serverEffectiveRemainingMinutes = input?.serverEffectiveRemainingMinutes;
  const timeCreditsConsumed = input?.timeCreditsConsumed ?? 0;
  const deductionProfile = input?.deductionProfile ?? null;
  const cafeTimezone = input?.cafeTimezone ?? DEFAULT_CAFE_TZ;
  const expiryDate = input?.expiryDate ?? null;

  useEffect(() => {
    if (!sessionStartTime) {
      walletRef.current = null;
      setLocalMinutes(null);
      return;
    }

    if (typeof walletBalanceMinutes === 'number') {
      walletRef.current = walletBalanceMinutes;
    } else if (typeof serverEffectiveRemainingMinutes === 'number') {
      walletRef.current = walletBalanceFromEffectiveRemaining(
        sessionStartTime,
        serverEffectiveRemainingMinutes,
        timeCreditsConsumed,
        deductionProfile,
        cafeTimezone,
      );
    } else {
      walletRef.current = null;
      setLocalMinutes(null);
      return;
    }

    const tick = () => {
      const wallet = walletRef.current;
      if (wallet === null) return;
      setLocalMinutes(
        capRemainingByExpiry(
          effectiveRemainingMinutes(
            sessionStartTime,
            wallet,
            timeCreditsConsumed,
            deductionProfile,
            cafeTimezone,
          ),
          expiryDate,
        ),
      );
    };

    tick();
    const id = setInterval(tick, SESSION_CLOCK_TICK_MS);
    return () => clearInterval(id);
  }, [
    sessionStartTime,
    walletBalanceMinutes,
    serverEffectiveRemainingMinutes,
    timeCreditsConsumed,
    deductionProfile,
    cafeTimezone,
    expiryDate,
  ]);

  return localMinutes;
}
