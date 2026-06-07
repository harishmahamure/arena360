import { DEFAULT_CAFE_TZ, type DeductionProfile } from '@gaming-cafe/contracts';
import { interpolateRemainingMinutes } from '@gaming-cafe/utils';
import { useEffect, useRef } from 'react';
import { useNotification } from './useNotification';

export interface CountdownConfig {
  id: string;
  /** Server-authoritative wallet minutes at last sync. */
  remainingMinutes: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  sessionDetails?: {
    playerName: string;
    deviceName: string;
  };
}

const THRESHOLDS_MIN = [10, 5, 1] as const;

export const useMultipleCountdowns = (configs: CountdownConfig[]) => {
  const { triggerNotification } = useNotification();

  const anchorsRef = useRef<Map<string, { remaining: number; syncedAt: number }>>(new Map());
  const notificationsSentRef = useRef<
    Map<string, Record<(typeof THRESHOLDS_MIN)[number], boolean>>
  >(new Map());
  const configKeyRef = useRef('');

  useEffect(() => {
    const configKey = configs
      .map((c) => `${c.id}:${c.remainingMinutes}`)
      .sort()
      .join('|');

    if (configKeyRef.current !== configKey) {
      configKeyRef.current = configKey;
      const now = Date.now();
      for (const config of configs) {
        anchorsRef.current.set(config.id, {
          remaining: config.remainingMinutes,
          syncedAt: now,
        });
        if (!notificationsSentRef.current.has(config.id)) {
          notificationsSentRef.current.set(config.id, {
            10: false,
            5: false,
            1: false,
          });
        }
      }
      const currentIds = new Set(configs.map((c) => c.id));
      for (const id of anchorsRef.current.keys()) {
        if (!currentIds.has(id)) {
          anchorsRef.current.delete(id);
          notificationsSentRef.current.delete(id);
        }
      }
    }
  }, [configs]);

  useEffect(() => {
    if (configs.length === 0) return;

    const interval = setInterval(() => {
      const now = Date.now();
      for (const config of configs) {
        const anchor = anchorsRef.current.get(config.id);
        if (!anchor) continue;

        const localRemaining = interpolateRemainingMinutes(
          anchor.remaining,
          anchor.syncedAt,
          now,
          config.deductionProfile,
          config.cafeTimezone ?? DEFAULT_CAFE_TZ,
        );

        const notifications = notificationsSentRef.current.get(config.id);
        if (!notifications) continue;

        for (const threshold of THRESHOLDS_MIN) {
          if (localRemaining <= threshold && !notifications[threshold]) {
            triggerNotification(
              `${config.sessionDetails?.playerName} has ${threshold} minute${threshold === 1 ? '' : 's'} remaining, device: ${config.sessionDetails?.deviceName}. Please ask player to resume session.`,
              `${config.id}-${threshold}min`,
            );
            notifications[threshold] = true;
          }
        }
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [configs, triggerNotification]);
};
