import { useCallback, useEffect, useRef } from 'react';
import { toast } from 'react-toastify';

interface WindowWithWebkitAudio extends Window {
  webkitAudioContext?: typeof AudioContext;
}

export const useNotification = () => {
  const audioContextRef = useRef<AudioContext | null>(null);
  const isAudioSupportedRef = useRef<boolean>(false);

  useEffect(() => {
    // Initialize Web Audio API context with error handling
    try {
      // Check if AudioContext is supported
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
      // Cleanup
      if (audioContextRef.current) {
        try {
          // Check if context is still open before closing
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
    // Early return if audio is not supported
    if (!isAudioSupportedRef.current || !audioContextRef.current) {
      return;
    }

    try {
      const context = audioContextRef.current;

      // Check if context is in a valid state
      if (context.state === 'closed') {
        return;
      }

      // Resume context if it's suspended (common on mobile devices)
      if (context.state === 'suspended') {
        context.resume().catch((_err) => {});
      }

      const oscillator = context.createOscillator();
      const gainNode = context.createGain();

      oscillator.connect(gainNode);
      gainNode.connect(context.destination);

      oscillator.type = 'sine'; // Sine wave for smooth sound
      oscillator.frequency.setValueAtTime(800, context.currentTime); // 800 Hz

      gainNode.gain.setValueAtTime(0, context.currentTime);
      gainNode.gain.linearRampToValueAtTime(0.3, context.currentTime + 0.05); // Fade in
      gainNode.gain.linearRampToValueAtTime(0.3, context.currentTime + 0.95); // Sustain
      gainNode.gain.linearRampToValueAtTime(0, context.currentTime + 1.0); // Fade out

      // Play the sound
      oscillator.start(context.currentTime);
      oscillator.stop(context.currentTime + 1.0); // 1000ms (1 second) duration
    } catch (_error) {}
  }, []);

  const triggerNotification = useCallback(
    (message: string, tag: string, sessionId?: string) => {
      playNotificationSound();

      toast.warning(message, {
        autoClose: 10000,
        position: 'bottom-center',
        style: {
          backgroundColor: '#FF6900',
          color: '#fff',
          borderRadius: '10px',
          padding: '10px',
          fontSize: '16px',
          fontWeight: 'bold',
          textAlign: 'center',
          boxShadow: '0 0 10px 0 rgba(0, 0, 0, 0.5)',
          cursor: sessionId ? 'pointer' : 'default',
        },
        onClick: sessionId
          ? () => {
              window.location.assign(`/sessions/${sessionId}`);
            }
          : undefined,
      });

      // Check if Notification API is supported
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
