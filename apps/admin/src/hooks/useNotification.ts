import { toastUtils } from '@gaming-cafe/utils';
import { useCallback, useEffect, useRef } from 'react';

interface WindowWithWebkitAudio extends Window {
  webkitAudioContext?: typeof AudioContext;
}

export const useNotification = () => {
  const audioContextRef = useRef<AudioContext | null>(null);
  const isAudioSupportedRef = useRef<boolean>(false);

  useEffect(() => {
    try {
      const AudioContextClass =
        window.AudioContext || (window as WindowWithWebkitAudio).webkitAudioContext;

      if (!AudioContextClass) {
        return;
      }

      audioContextRef.current = new AudioContextClass();
      isAudioSupportedRef.current = true;
    } catch (_error) {
      isAudioSupportedRef.current = false;
    }

    return () => {
      if (audioContextRef.current) {
        try {
          if (audioContextRef.current.state !== 'closed') {
            audioContextRef.current.close();
          }
        } catch (_error) {
        } finally {
          audioContextRef.current = null;
          isAudioSupportedRef.current = false;
        }
      }
    };
  }, []);

  const playNotificationSound = useCallback(() => {
    if (!isAudioSupportedRef.current || !audioContextRef.current) {
      return;
    }

    try {
      const context = audioContextRef.current;

      if (context.state === 'closed') {
        return;
      }

      if (context.state === 'suspended') {
        context.resume().catch((_err) => {});
      }

      const oscillator = context.createOscillator();
      const gainNode = context.createGain();

      oscillator.connect(gainNode);
      gainNode.connect(context.destination);

      oscillator.type = 'sine';
      oscillator.frequency.setValueAtTime(800, context.currentTime);

      gainNode.gain.setValueAtTime(0, context.currentTime);
      gainNode.gain.linearRampToValueAtTime(0.3, context.currentTime + 0.05);
      gainNode.gain.linearRampToValueAtTime(0.3, context.currentTime + 0.95);
      gainNode.gain.linearRampToValueAtTime(0, context.currentTime + 1.0);

      oscillator.start(context.currentTime);
      oscillator.stop(context.currentTime + 1.0);
    } catch (_error) {}
  }, []);

  const triggerNotification = useCallback(
    (message: string, tag: string, sessionId?: string) => {
      playNotificationSound();

      toastUtils.sessionWarning(message, { sessionId, tag });

      if ('Notification' in window) {
        try {
          if (Notification.permission === 'granted') {
            new Notification(message, {
              body: message,
              icon: '/favicon.ico',
              tag,
              requireInteraction: true,
              silent: false,
            });
          }
        } catch (_error) {}
      }
    },
    [playNotificationSound],
  );

  useEffect(() => {
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission().catch((_error) => {});
    }
  }, []);

  return { triggerNotification };
};
