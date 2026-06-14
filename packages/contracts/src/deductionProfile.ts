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

/** Seconds remaining on the HUD when kiosk/console auto-call session end. */
export const AUTO_END_REMAINING_SECONDS = 10;

/**
 * Wallet minutes consumed between two UTC instants using the plan profile.
 * Mirrors backend `weighted_minutes_between` (minute-segment integration).
 */
export function weightedMinutesBetween(
  startMs: number,
  endMs: number,
  profile: DeductionProfile,
  timeZone: string,
): number {
  if (endMs <= startMs) return 0;

  let total = 0;
  let cursor = startMs;
  while (cursor < endMs) {
    const ratio = currentDeductionRatio(profile, timeZone, new Date(cursor));
    const nextMinute = cursor + 60_000;
    const segmentEnd = Math.min(nextMinute, endMs);
    const secs = (segmentEnd - cursor) / 1000;
    total += (secs / 60) * ratio;
    cursor = segmentEnd;
  }
  return total;
}

/** Project wallet minutes left for an open session (display only). */
export function effectiveRemainingMinutes(
  sessionStartTime: string,
  walletBalanceMinutes: number,
  timeCreditsConsumed: number,
  deductionProfile: DeductionProfile | null | undefined,
  cafeTimezone: string,
  nowMs = Date.now(),
): number {
  const startMs = Date.parse(sessionStartTime);
  if (Number.isNaN(startMs)) return walletBalanceMinutes;

  const consumed =
    deductionProfile != null
      ? weightedMinutesBetween(startMs, nowMs, deductionProfile, cafeTimezone)
      : (nowMs - startMs) / 60_000;

  const owed = Math.max(0, consumed - timeCreditsConsumed);
  return Math.max(0, walletBalanceMinutes - owed);
}

/** Derive wallet balance from server effective remaining at a sync instant. */
export function walletBalanceFromEffectiveRemaining(
  sessionStartTime: string,
  effectiveRemainingMinutes: number,
  timeCreditsConsumed: number,
  deductionProfile: DeductionProfile | null | undefined,
  cafeTimezone: string,
  syncedAtMs = Date.now(),
): number {
  const startMs = Date.parse(sessionStartTime);
  if (Number.isNaN(startMs)) return effectiveRemainingMinutes;

  const consumed =
    deductionProfile != null
      ? weightedMinutesBetween(startMs, syncedAtMs, deductionProfile, cafeTimezone)
      : (syncedAtMs - startMs) / 60_000;

  const owed = Math.max(0, consumed - timeCreditsConsumed);
  return effectiveRemainingMinutes + owed;
}

export function deductionPeriodLabelForNow(
  profile: DeductionProfile,
  timeZone: string,
  now = new Date(),
): ReturnType<typeof deductionPeriodLabel> {
  return deductionPeriodLabel(currentDeductionRatio(profile, timeZone, now));
}

export type DeductionPlayPeriod = 'peak' | 'low' | 'normal';

export interface DeductionPlayRow {
  period: DeductionPlayPeriod;
  label: string;
  timeRange: string;
  ratio: number;
  walletMinutes: number;
  wallPlayMinutes: number;
}

/** Formats `HH:mm` or `HH:mm:ss` as 12-hour local time (e.g. `23:00:00` → `11 PM`). */
export function formatDeductionTime(value: string): string {
  const totalMinutes = parseTimeToMinutes(value);
  const hours24 = Math.floor(totalMinutes / 60) % 24;
  const minutes = totalMinutes % 60;
  const period = hours24 >= 12 ? 'PM' : 'AM';
  const hours12 = hours24 % 12 === 0 ? 12 : hours24 % 12;
  const minutePart = minutes === 0 ? '' : `:${minutes.toString().padStart(2, '0')}`;
  return `${hours12}${minutePart} ${period}`;
}

/** Human-readable window label (e.g. `11 PM – 6 AM`). */
export function formatDeductionTimeRange(start: string, end: string): string {
  return `${formatDeductionTime(start)} – ${formatDeductionTime(end)}`;
}

/** Playable wall-clock minutes per deduction period for a wallet balance. */
export function buildDeductionPlayBreakdown(
  walletMinutes: number,
  profile: DeductionProfile,
): DeductionPlayRow[] {
  const credits = walletMinutes > 0 ? walletMinutes : 0;

  return [
    {
      period: 'low',
      label: 'Low hours',
      timeRange: formatDeductionTimeRange(profile.lowWindowStart, profile.lowWindowEnd),
      ratio: profile.lowRatio,
      walletMinutes: credits,
      wallPlayMinutes: credits > 0 ? maxWallMinutes(credits, profile.lowRatio) : 0,
    },
    {
      period: 'normal',
      label: 'Normal hours',
      timeRange: 'All other hours',
      ratio: 1,
      walletMinutes: credits,
      wallPlayMinutes: credits,
    },
    {
      period: 'peak',
      label: 'Peak hours',
      timeRange: formatDeductionTimeRange(profile.peakWindowStart, profile.peakWindowEnd),
      ratio: profile.peakRatio,
      walletMinutes: credits,
      wallPlayMinutes: credits > 0 ? maxWallMinutes(credits, profile.peakRatio) : 0,
    },
  ];
}
