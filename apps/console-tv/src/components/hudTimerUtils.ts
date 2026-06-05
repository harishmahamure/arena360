export function formatClock(totalSeconds: number): string {
  const safe = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const seconds = safe % 60;
  const pad = (n: number) => n.toString().padStart(2, '0');
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}

export type HudTone = 'normal' | 'warning' | 'critical';

export function toneForMinutes(minutesLeft: number): HudTone {
  if (minutesLeft <= 1) return 'critical';
  if (minutesLeft <= 5) return 'warning';
  return 'normal';
}
