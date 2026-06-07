import type { DeductionProfile } from '@gaming-cafe/contracts';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useSessionRemainingMinutes } from './useSessionRemainingMinutes';

const profile: DeductionProfile = {
  peakWindowStart: '18:00:00',
  peakWindowEnd: '23:00:00',
  peakRatio: 2,
  lowWindowStart: '07:00:00',
  lowWindowEnd: '11:00:00',
  lowRatio: 0.5,
};

describe('useSessionRemainingMinutes', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-07T13:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('ticks 1:1 without a deduction profile', () => {
    const { result } = renderHook(() => useSessionRemainingMinutes(10));
    expect(result.current).toBe(10);

    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(result.current).toBeCloseTo(9, 5);
  });

  it('burns faster during peak windows', () => {
    const { result } = renderHook(() => useSessionRemainingMinutes(10, profile, 'Asia/Kolkata'));
    expect(result.current).toBe(10);

    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(result.current).toBeCloseTo(8, 5);
  });

  it('re-anchors when the authoritative value changes', () => {
    const { result, rerender } = renderHook(({ minutes }) => useSessionRemainingMinutes(minutes), {
      initialProps: { minutes: 10 },
    });

    act(() => {
      vi.advanceTimersByTime(120_000);
    });
    expect(result.current).toBeCloseTo(8, 5);

    rerender({ minutes: 25 });
    expect(result.current).toBe(25);
  });
});
