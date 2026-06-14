import {
  capRemainingByExpiry,
  DEFAULT_CAFE_TZ,
  type DeductionProfile,
  effectiveRemainingMinutes,
  SESSION_CLOCK_TICK_MS,
} from '@gaming-cafe/contracts';
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

const THRESHOLDS_MIN = [10, 5, 1] as const;

export const useMultipleCountdowns = (configs: CountdownConfig[]) => {
  const { triggerNotification } = useNotification();

  const notificationsSentRef = useRef<
    Map<string, Record<(typeof THRESHOLDS_MIN)[number], boolean>>
  >(new Map());
  const configKeyRef = useRef('');

  useEffect(() => {
    const configKey = configs
      .map((c) => `${c.id}:${c.remainingMinutes}:${c.sessionStartTime}`)
      .sort()
      .join('|');

    if (configKeyRef.current !== configKey) {
      configKeyRef.current = configKey;
      for (const config of configs) {
        if (!notificationsSentRef.current.has(config.id)) {
          notificationsSentRef.current.set(config.id, {
            10: false,
            5: false,
            1: false,
          });
        }
      }
      const currentIds = new Set(configs.map((c) => c.id));
      for (const id of notificationsSentRef.current.keys()) {
        if (!currentIds.has(id)) {
          notificationsSentRef.current.delete(id);
        }
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

        const notifications = notificationsSentRef.current.get(config.id);
        if (!notifications) continue;

        for (const threshold of THRESHOLDS_MIN) {
          if (localRemaining <= threshold && !notifications[threshold]) {
            triggerNotification(
              `${config.sessionDetails?.playerName} has ${threshold} minute${threshold === 1 ? '' : 's'} remaining on ${config.sessionDetails?.deviceName}. Tap to view session.`,
              `${config.id}-${threshold}min`,
              config.id,
            );
            notifications[threshold] = true;
          }
        }
      }
    }, SESSION_CLOCK_TICK_MS);

    return () => clearInterval(interval);
  }, [configs, triggerNotification]);
};
