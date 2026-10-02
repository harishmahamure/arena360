import { describe, expect, it } from 'vitest';
import { fixture } from './analyticsFixture';
import {
  capacityModel,
  defaultAssumptions,
  forecast,
  pricingScenario,
  ratio,
  summarize,
} from './analyticsModel';

describe('business metric definitions', () => {
  it('separates comparison periods and includes zero-sales days without averaging daily averages', () => {
    const result = summarize(fixture);
    expect(result.daily).toHaveLength(14);
    expect(result.revenue).toBe(100);
    expect(result.priorRevenue).toBe(50);
    expect(result.growth).toBe(1);
    expect(result.averageTicket).toBe(50);
    expect(ratio(100, 0)).toBeNull();
  });
  it('accounts for every selected weekday occurrence and clips partial hours in IST', () => {
    const result = capacityModel(fixture, { ...defaultAssumptions, openHour: 10, closeHour: 11 });
    expect(result.available).toBe(14);
    expect(result.used).toBe(1);
    expect(result.unused).toBe(13);
    expect(result.opportunity).toBe(325);
    const partial = {
      ...fixture,
      period: { ...fixture.period, endDate: '2026-09-01', observedUntil: '2026-09-01T05:00:00Z' },
      hourlyUsage: [],
    };
    expect(
      capacityModel(partial, { ...defaultAssumptions, openHour: 10, closeHour: 11 }).available,
    ).toBe(0.5);
  });
  it('flags impossible occupancy instead of claiming negative lost capacity', () => {
    const data = {
      ...fixture,
      hourlyUsage: [{ date: '2026-09-01', weekday: 2, hour: 10, hours: 1000, starts: 5 }],
    };
    const result = capacityModel(data, defaultAssumptions);
    expect(result.inconsistent).toBe(true);
    expect(result.unused).toBe(0);
  });
  it('caps price scenarios at physical capacity and never infers elasticity', () => {
    expect(pricingScenario(90, 100, 100, 10, 50)).toEqual({
      baseline: 9000,
      projected: 11000,
      delta: 2000,
      projectedHours: 100,
    });
    expect(pricingScenario(90, 100, 100, 10, -100).projected).toBe(0);
  });
  it('requires 14 complete days, includes zero days, and forecasts after the selected end', () => {
    const result = forecast(fixture);
    expect(result).toHaveLength(7);
    expect(result[0]?.date).toBe('2026-09-15');
    expect(result[0]?.revenue).toBe(50);
    expect(result[0]?.samples).toBe(2);
    expect(result[0]?.low).toBe(0);
    expect(
      forecast({
        ...fixture,
        period: { ...fixture.period, observedUntil: '2026-09-14T12:00:00Z' },
      }),
    ).toEqual([]);
  });
});
