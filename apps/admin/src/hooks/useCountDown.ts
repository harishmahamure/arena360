import { useEffect, useRef, useState } from 'react';
import { useNotification } from './useNotification';

export interface CountdownConfig {
  id: string;
  remainingTime: number;
  onTimeExpired?: () => void;
  beforeExpiredTime?: number;
  sessionDetails?: {
    playerName: string;
    deviceName: string;
  };
}

export const useMultipleCountdowns = (configs: CountdownConfig[]) => {
  const [countdowns, setCountdowns] = useState<CountdownConfig[]>([]);
  const { triggerNotification } = useNotification();

  const notificationsSentRef = useRef<
    Map<
      string,
      {
        tenMinute: boolean;
        fiveMinute: boolean;
        oneMinute: boolean;
      }
    >
  >(new Map());

  const configIdsRef = useRef<string>('');

  useEffect(() => {
    const newConfigIds = configs
      .map((c) => c.id)
      .sort()
      .join(',');

    if (configIdsRef.current !== newConfigIds) {
      configIdsRef.current = newConfigIds;

      setCountdowns(
        configs.map((config) => ({
          ...config,
          remainingTime: Math.floor(config.remainingTime),
        })),
      );

      configs.forEach((config) => {
        if (!notificationsSentRef.current.has(config.id)) {
          notificationsSentRef.current.set(config.id, {
            tenMinute: false,
            fiveMinute: false,
            oneMinute: false,
          });
        }
      });

      const currentIds = new Set(configs.map((c) => c.id));
      for (const id of notificationsSentRef.current.keys()) {
        if (!currentIds.has(id)) {
          notificationsSentRef.current.delete(id);
        }
      }
    }
  }, [configs]);

  useEffect(() => {
    if (countdowns.length === 0) return;

    const interval = setInterval(() => {
      setCountdowns((prev) => {
        return prev.map((config) => {
          if (config.remainingTime > 0) {
            const notifications = notificationsSentRef.current.get(config.id);

            if (notifications) {
              if (config.remainingTime <= 600 && !notifications.tenMinute) {
                triggerNotification(
                  `${config.sessionDetails?.playerName} has 10 minutes remaining, device: ${config.sessionDetails?.deviceName}. Please ask player to resume session.`,
                  `${config.id}-10min`,
                );
                notifications.tenMinute = true;
              }

              if (config.remainingTime <= 300 && !notifications.fiveMinute) {
                triggerNotification(
                  `${config.sessionDetails?.playerName} has 5 minutes remaining, device: ${config.sessionDetails?.deviceName}. Please ask player to resume session.`,
                  `${config.id}-5min`,
                );
                notifications.fiveMinute = true;
              }

              if (config.remainingTime <= 60 && !notifications.oneMinute) {
                triggerNotification(
                  `${config.sessionDetails?.playerName} has 1 minute remaining, device: ${config.sessionDetails?.deviceName}. Please ask player to resume session.`,
                  `${config.id}-1min`,
                );
                notifications.oneMinute = true;
              }
            }

            return {
              ...config,
              remainingTime: config.remainingTime - 1,
            };
          }
          return config;
        });
      });
    }, 1000);

    return () => clearInterval(interval);
  }, [countdowns.length, triggerNotification]);

  return countdowns;
};
