/** Format wallet-remaining minutes as `H:MM:SS` or `M:SS`. */
export function formatRemainingClock(remainingMinutes: number | null | undefined): string {
  if (remainingMinutes === null || remainingMinutes === undefined) return '--:--';
  const totalSeconds = Math.max(0, Math.floor(remainingMinutes * 60));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (n: number) => n.toString().padStart(2, '0');
  if (hours > 0) return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  return `${minutes}:${pad(seconds)}`;
}

/** Long-form label for admin tables (`2h 15m 30s`). */
export function formatRemainingLabel(remainingMinutes: number | null | undefined): string {
  if (remainingMinutes === null || remainingMinutes === undefined) return 'N/A';
  const totalSeconds = Math.max(0, Math.floor(remainingMinutes * 60));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (totalSeconds <= 0) return 'Expired';
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}
