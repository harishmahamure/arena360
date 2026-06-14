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
    const start = '2026-06-07T13:00:00.000Z';
    vi.setSystemTime(new Date(start));
    const { result } = renderHook(() =>
      useSessionRemainingMinutes({
        sessionStartTime: start,
        walletBalanceMinutes: 10,
      }),
    );
    expect(result.current).toBe(10);

    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(result.current).toBeCloseTo(9, 5);
  });

  it('burns faster during peak windows', () => {
    const start = '2026-06-07T12:30:00.000Z';
    vi.setSystemTime(new Date(start));
    const { result } = renderHook(() =>
      useSessionRemainingMinutes({
        sessionStartTime: start,
        walletBalanceMinutes: 10,
        deductionProfile: profile,
        cafeTimezone: 'Asia/Kolkata',
      }),
    );
    expect(result.current).toBe(10);

    act(() => {
      vi.advanceTimersByTime(60_000);
    });
    expect(result.current).toBeCloseTo(8, 5);
  });

  it('re-anchors when wallet balance changes', () => {
    const start = '2026-06-07T13:00:00.000Z';
    vi.setSystemTime(new Date(start));
    const { result, rerender } = renderHook(
      ({ walletBalanceMinutes }: { walletBalanceMinutes: number }) =>
        useSessionRemainingMinutes({
          sessionStartTime: start,
          walletBalanceMinutes,
        }),
      { initialProps: { walletBalanceMinutes: 10 } },
    );

    act(() => {
      vi.advanceTimersByTime(120_000);
    });
    expect(result.current).toBeCloseTo(8, 5);

    rerender({ walletBalanceMinutes: 25 });
    expect(result.current).toBeCloseTo(23, 5);
  });
});
