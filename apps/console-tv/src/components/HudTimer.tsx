import { useEffect, useRef, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { formatClock, toneForMinutes } from './hudTimerUtils';

interface HudTimerProps {
  /** Server-authoritative remaining minutes; resets the local countdown. */
  remainingMinutes: number;
}

const TONE_COLORS = {
  normal: '#4ade80',
  warning: '#fbbf24',
  critical: '#f87171',
} as const;

/**
 * Always-visible countdown. Ticks locally every second and re-anchors when
 * `remainingMinutes` changes (WebSocket / balance.updated).
 */
export function HudTimer({ remainingMinutes }: HudTimerProps) {
  const deadlineRef = useRef<number>(Date.now() + remainingMinutes * 60_000);
  const [secondsLeft, setSecondsLeft] = useState(() => remainingMinutes * 60);

  useEffect(() => {
    deadlineRef.current = Date.now() + remainingMinutes * 60_000;
    setSecondsLeft(remainingMinutes * 60);
  }, [remainingMinutes]);

  useEffect(() => {
    const id = setInterval(() => {
      setSecondsLeft(Math.max(0, (deadlineRef.current - Date.now()) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, []);

  const tone = toneForMinutes(secondsLeft / 60);

  return (
    <View style={styles.container} accessibilityLiveRegion="polite">
      <Text style={styles.label}>Time remaining</Text>
      <Text style={[styles.value, { color: TONE_COLORS[tone] }]}>{formatClock(secondsLeft)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    alignItems: 'center',
    paddingVertical: 16,
    paddingHorizontal: 32,
    backgroundColor: 'rgba(0,0,0,0.72)',
    borderRadius: 12,
  },
  label: {
    color: '#94a3b8',
    fontSize: 18,
    marginBottom: 4,
  },
  value: {
    fontSize: 56,
    fontWeight: '700',
    fontVariant: ['tabular-nums'],
  },
});
