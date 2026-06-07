/** Peak consumption must exceed 1 wallet minute per wall minute. */
export const PEAK_RATIO_MIN = 1.01;

/** Low consumption must be below 1 wallet minute per wall minute. */
export const LOW_RATIO_MAX = 0.99;

export const LOW_RATIO_MIN = 0.01;

export interface DeductionProfile {
  peakWindowStart: string;
  peakWindowEnd: string;
  peakRatio: number;
  lowWindowStart: string;
  lowWindowEnd: string;
  lowRatio: number;
}

const TIME_RE = /^([0-1]?[0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$/;

export function parseTimeToMinutes(value: string): number {
  const parts = value.split(':').map((p) => Number.parseInt(p, 10));
  const [h = 0, m = 0] = parts;
  return h * 60 + m;
}

export function isValidTimeString(value: string): boolean {
  return TIME_RE.test(value);
}

/** Whether `minuteOfDay` (0–1439) falls in [start, end), supporting wrap-around. */
export function minuteInWindow(minuteOfDay: number, start: string, end: string): boolean {
  const s = parseTimeToMinutes(start);
  const e = parseTimeToMinutes(end);
  if (s === e) return false;
  if (s < e) return minuteOfDay >= s && minuteOfDay < e;
  return minuteOfDay >= s || minuteOfDay < e;
}

export function ratioAtMinute(minuteOfDay: number, profile: DeductionProfile): number {
  if (minuteInWindow(minuteOfDay, profile.peakWindowStart, profile.peakWindowEnd)) {
    return profile.peakRatio;
  }
  if (minuteInWindow(minuteOfDay, profile.lowWindowStart, profile.lowWindowEnd)) {
    return profile.lowRatio;
  }
  return 1;
}

export function windowsOverlap(profile: DeductionProfile): boolean {
  for (let minute = 0; minute < 24 * 60; minute += 1) {
    const inPeak = minuteInWindow(minute, profile.peakWindowStart, profile.peakWindowEnd);
    const inLow = minuteInWindow(minute, profile.lowWindowStart, profile.lowWindowEnd);
    if (inPeak && inLow) return true;
  }
  return false;
}

export function validateDeductionProfile(profile: DeductionProfile): string | null {
  for (const field of [
    profile.peakWindowStart,
    profile.peakWindowEnd,
    profile.lowWindowStart,
    profile.lowWindowEnd,
  ]) {
    if (!isValidTimeString(field)) return `Invalid time format: ${field}`;
  }
  if (profile.peakRatio <= 1) return 'Peak ratio must be greater than 1';
  if (profile.lowRatio <= 0 || profile.lowRatio >= 1) {
    return 'Low ratio must be between 0 and 1 (exclusive)';
  }
  if (windowsOverlap(profile)) return 'Peak and low windows must not overlap';
  return null;
}

/** Max wall-clock minutes playable from a wallet balance in a single period. */
export function maxWallMinutes(walletMinutes: number, ratio: number): number {
  if (ratio <= 0) return 0;
  return walletMinutes / ratio;
}

export function deductionPeriodLabel(ratio: number): 'peak' | 'low' | 'normal' {
  if (ratio > 1) return 'peak';
  if (ratio < 1) return 'low';
  return 'normal';
}

/** Default venue IANA timezone (`CAFE_TZ` backend default). */
export const DEFAULT_CAFE_TZ = 'Asia/Kolkata';

/** Venue-local minute-of-day (0–1439) from a Date and IANA timezone. */
export function localMinuteOfDay(date: Date, timeZone: string): number {
  const parts = new Intl.DateTimeFormat('en-GB', {
    timeZone,
    hour: 'numeric',
    minute: 'numeric',
    hour12: false,
  }).formatToParts(date);
  const hour = Number(parts.find((p) => p.type === 'hour')?.value ?? 0);
  const minute = Number(parts.find((p) => p.type === 'minute')?.value ?? 0);
  return hour * 60 + minute;
}

export function currentDeductionRatio(
  profile: DeductionProfile,
  timeZone: string,
  now = new Date(),
): number {
  return ratioAtMinute(localMinuteOfDay(now, timeZone), profile);
}

export function deductionPeriodLabelForNow(
  profile: DeductionProfile,
  timeZone: string,
  now = new Date(),
): ReturnType<typeof deductionPeriodLabel> {
  return deductionPeriodLabel(currentDeductionRatio(profile, timeZone, now));
}
