import { describe, expect, it } from 'vitest';
import { calculatePeriodChange, normalizeRevenue } from './statsHelpers';

describe('normalizeRevenue', () => {
  it('defaults missing fields to zero', () => {
    const result = normalizeRevenue({ plan: 55 });

    expect(result.plan).toBe(55);
    expect(result.creditRevenue).toBe(0);
    expect(result.planCashRevenue).toBe(0);
    expect(result.planCashCount).toBe(0);
    expect(result.total).toBe(55);
  });

  it('derives plan and merchandise from payment breakdown when present', () => {
    const result = normalizeRevenue({
      plan: 660,
      merchandise: 495,
      total: 2020,
      planCashRevenue: 360,
      planOnlineRevenue: 300,
      planCreditRevenue: 480,
      productCashRevenue: 300,
      productOnlineRevenue: 195,
      productCreditRevenue: 385,
    });

    expect(result.plan).toBe(1140);
    expect(result.merchandise).toBe(880);
    expect(result.total).toBe(2020);
  });
});

describe('calculatePeriodChange', () => {
  it('returns 100 when previous is zero and current is positive', () => {
    expect(calculatePeriodChange(55, 0)).toEqual({ value: 100, positive: true });
  });

  it('returns 0 when both periods are zero', () => {
    expect(calculatePeriodChange(0, 0)).toEqual({ value: 0, positive: true });
  });
});
