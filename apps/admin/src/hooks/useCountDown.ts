import {
  capRemainingByExpiry,
  DEFAULT_CAFE_TZ,
  type DeductionProfile,
  effectiveRemainingMinutes,
  SESSION_CLOCK_TICK_MS,
} from '@gaming-cafe/contracts';
import {
  getSessionUrgentThreshold,
  type SessionUrgentThreshold,
  shouldEmitSessionWarning,
} from '@gaming-cafe/utils';
import { useEffect, useRef } from 'react';
import { useNotification } from './useNotification';

export interface CountdownConfig {
  id: string;
  sessionStartTime: string;
  /** Server-authoritative wallet minutes at last sync. */
  remainingMinutes: number;
  timeCreditsConsumed?: number | null;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  expiryDate?: string | null;
  sessionDetails?: {
    playerName: string;
    deviceName: string;
  };
}

export const useMultipleCountdowns = (configs: CountdownConfig[]) => {
  const { triggerNotification } = useNotification();

  const lastUrgentBandRef = useRef<Map<string, SessionUrgentThreshold | null>>(new Map());
  const lastWalletMinutesRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    for (const config of configs) {
      const prevWallet = lastWalletMinutesRef.current.get(config.id);
      if (prevWallet !== undefined && config.remainingMinutes > prevWallet + 0.05) {
        lastUrgentBandRef.current.delete(config.id);
      }
      lastWalletMinutesRef.current.set(config.id, config.remainingMinutes);
    }

    const currentIds = new Set(configs.map((c) => c.id));
    for (const id of lastUrgentBandRef.current.keys()) {
      if (!currentIds.has(id)) {
        lastUrgentBandRef.current.delete(id);
      }
    }
    for (const id of lastWalletMinutesRef.current.keys()) {
      if (!currentIds.has(id)) {
        lastWalletMinutesRef.current.delete(id);
      }
    }
  }, [configs]);

  useEffect(() => {
    if (configs.length === 0) return;

    const interval = setInterval(() => {
      for (const config of configs) {
        const localRemaining = capRemainingByExpiry(
          effectiveRemainingMinutes(
            config.sessionStartTime,
            config.remainingMinutes,
            config.timeCreditsConsumed ?? 0,
            config.deductionProfile,
            config.cafeTimezone ?? DEFAULT_CAFE_TZ,
          ),
          config.expiryDate,
        );

        const nextBand = getSessionUrgentThreshold(localRemaining);
        const prevBand = lastUrgentBandRef.current.get(config.id);

        if (nextBand === null) {
          lastUrgentBandRef.current.set(config.id, null);
          continue;
        }

        if (!shouldEmitSessionWarning(prevBand, nextBand)) {
          continue;
        }

        triggerNotification(
          `${config.sessionDetails?.playerName} has ${nextBand} minute${nextBand === 1 ? '' : 's'} remaining on ${config.sessionDetails?.deviceName}. Tap to view session.`,
          `session-${config.id}`,
          config.id,
        );
        lastUrgentBandRef.current.set(config.id, nextBand);
      }
    }, SESSION_CLOCK_TICK_MS);

    return () => clearInterval(interval);
  }, [configs, triggerNotification]);
};
