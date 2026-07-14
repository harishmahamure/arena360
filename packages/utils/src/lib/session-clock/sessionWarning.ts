/** Admin session countdown warning bands (minutes). Most urgent checked first. */
export const SESSION_WARNING_THRESHOLDS = [10, 5, 1] as const;

export type SessionUrgentThreshold = (typeof SESSION_WARNING_THRESHOLDS)[number];

/**
 * Returns the most urgent warning band for the current remaining time, or null when above all bands.
 */
export function getSessionUrgentThreshold(remainingMinutes: number): SessionUrgentThreshold | null {
  if (remainingMinutes <= 0) return null;
  if (remainingMinutes <= 1) return 1;
  if (remainingMinutes <= 5) return 5;
  if (remainingMinutes <= 10) return 10;
  return null;
}

/**
 * True when entering a new urgent band (including first observation on page load).
 */
export function shouldEmitSessionWarning(
  prevBand: SessionUrgentThreshold | null | undefined,
  nextBand: SessionUrgentThreshold | null,
): boolean {
  if (nextBand === null) return false;
  return prevBand !== nextBand;
}
