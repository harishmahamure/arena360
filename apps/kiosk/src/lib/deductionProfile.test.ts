import type { DeductionProfile } from '@gaming-cafe/contracts';
import { describe, expect, it } from 'vitest';
import { currentDeductionRatio, localMinuteOfDay } from './deductionProfile';

const profile: DeductionProfile = {
  peakWindowStart: '18:00:00',
  peakWindowEnd: '23:00:00',
  peakRatio: 1.5,
  lowWindowStart: '07:00:00',
  lowWindowEnd: '11:00:00',
  lowRatio: 0.8,
};

describe('deductionProfile', () => {
  it('detects peak ratio in evening local time', () => {
    const evening = new Date('2026-06-07T13:00:00.000Z');
    expect(currentDeductionRatio(profile, 'Asia/Kolkata', evening)).toBe(1.5);
  });

  it('detects low ratio in morning local time', () => {
    const morning = new Date('2026-06-07T03:00:00.000Z');
    expect(currentDeductionRatio(profile, 'Asia/Kolkata', morning)).toBe(0.8);
  });

  it('uses normal ratio outside configured windows', () => {
    const afternoon = new Date('2026-06-07T08:00:00.000Z');
    expect(currentDeductionRatio(profile, 'Asia/Kolkata', afternoon)).toBe(1);
  });

  it('parses local minute of day', () => {
    const morning = new Date('2026-06-07T03:00:00.000Z');
    expect(localMinuteOfDay(morning, 'Asia/Kolkata')).toBe(8 * 60 + 30);
  });
});
