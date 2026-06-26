import { describe, expect, it } from 'vitest';
import {
  buildDeductionPlayBreakdown,
  capRemainingByExpiry,
  createSessionClockCache,
  formatDeductionTime,
  formatDeductionTimeRange,
  maxWallMinutes,
  minutesUntilExpiry,
  ratioAtMinute,
  tickSessionClockCache,
  validateDeductionProfile,
  weightedMinutesBetween,
  windowsOverlap,
} from './deductionProfile';

const profile = {
  peakWindowStart: '18:00:00',
  peakWindowEnd: '23:00:00',
  peakRatio: 1.5,
  lowWindowStart: '07:00:00',
  lowWindowEnd: '11:00:00',
  lowRatio: 0.8,
};

const posProfile = {
  peakWindowStart: '23:00:00',
  peakWindowEnd: '06:00:00',
  peakRatio: 2,
  lowWindowStart: '07:00:00',
  lowWindowEnd: '11:00:00',
  lowRatio: 0.5,
};

describe('deductionProfile contracts', () => {
  it('validates a coherent profile', () => {
    expect(validateDeductionProfile(profile)).toBeNull();
  });

  it('rejects overlapping windows', () => {
    expect(
      windowsOverlap({
        ...profile,
        lowWindowStart: '20:00:00',
        lowWindowEnd: '22:00:00',
      }),
    ).toBe(true);
  });

  it('returns peak ratio during evening minutes', () => {
    expect(ratioAtMinute(19 * 60, profile)).toBe(1.5);
  });

  it('formats deduction times without trailing :00', () => {
    expect(formatDeductionTime('23:00:00')).toBe('11 PM');
    expect(formatDeductionTime('07:30:00')).toBe('7:30 AM');
    expect(formatDeductionTime('06:00')).toBe('6 AM');
  });

  it('formats wrap-around time ranges', () => {
    expect(formatDeductionTimeRange('23:00:00', '06:00:00')).toBe('11 PM – 6 AM');
    expect(formatDeductionTimeRange('07:00:00', '11:00:00')).toBe('7 AM – 11 AM');
  });

  it('builds play breakdown with wall minutes per period', () => {
    const rows = buildDeductionPlayBreakdown(30, posProfile);
    const peak = rows.find((r) => r.period === 'peak');
    const low = rows.find((r) => r.period === 'low');
    const normal = rows.find((r) => r.period === 'normal');

    expect(peak?.wallPlayMinutes).toBe(maxWallMinutes(30, 2));
    expect(low?.wallPlayMinutes).toBe(maxWallMinutes(30, 0.5));
    expect(normal?.wallPlayMinutes).toBe(30);
    expect(peak?.timeRange).toBe('11 PM – 6 AM');
    expect(low?.timeRange).toBe('7 AM – 11 AM');
    expect(normal?.timeRange).toBe('All other hours');
  });

  it('weightedMinutesBetween matches low-window burn (~0.8 per wall minute)', () => {
    const start = Date.parse('2026-06-07T02:30:00.000Z');
    const end = start + 60 * 60_000;
    const weighted = weightedMinutesBetween(start, end, profile, 'Asia/Kolkata');
    expect(weighted).toBeCloseTo(48, 0);
  });

  it('minutesUntilExpiry floors remaining wall minutes', () => {
    const now = Date.parse('2026-06-07T12:00:00.000Z');
    const expiry = new Date(now + 15 * 60_000 + 30_000).toISOString();
    expect(minutesUntilExpiry(expiry, now)).toBe(15);
    expect(minutesUntilExpiry(expiry, now + 20 * 60_000)).toBe(0);
  });

  it('capRemainingByExpiry uses min(wallet, minutesUntilExpiry)', () => {
    const now = Date.parse('2026-06-07T12:00:00.000Z');
    const expirySoon = new Date(now + 15 * 60_000).toISOString();
    const expiryLater = new Date(now + 7 * 24 * 60 * 60_000).toISOString();

    expect(capRemainingByExpiry(300, expiryLater, now)).toBe(300);
    expect(capRemainingByExpiry(300, expirySoon, now)).toBe(15);
    expect(capRemainingByExpiry(10, expiryLater, now)).toBe(10);
    expect(capRemainingByExpiry(50, undefined, now)).toBe(50);
  });

  it('session clock cache ticks incrementally in O(1) per second', () => {
    const startMs = Date.parse('2026-06-07T02:30:00.000Z');
    const start = new Date(startMs).toISOString();
    const cache = createSessionClockCache(start, 60, 0, profile, 'Asia/Kolkata', null, startMs);
    expect(cache).not.toBeNull();
    const ticked = tickSessionClockCache(cache!, 1_000, startMs + 1_000);
    expect(ticked.remainingMinutes).toBeLessThan(60);
    expect(ticked.lastTickMs).toBe(startMs + 1_000);
  });
});
