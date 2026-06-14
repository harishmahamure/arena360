import { describe, expect, it } from 'vitest';
import {
  buildDeductionPlayBreakdown,
  formatDeductionTime,
  formatDeductionTimeRange,
  maxWallMinutes,
  ratioAtMinute,
  validateDeductionProfile,
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
});
