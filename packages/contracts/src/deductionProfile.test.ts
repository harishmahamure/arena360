import { describe, expect, it } from 'vitest';
import { ratioAtMinute, validateDeductionProfile, windowsOverlap } from './deductionProfile';

const profile = {
  peakWindowStart: '18:00:00',
  peakWindowEnd: '23:00:00',
  peakRatio: 1.5,
  lowWindowStart: '07:00:00',
  lowWindowEnd: '11:00:00',
  lowRatio: 0.8,
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
});
