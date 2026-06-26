import {
  capRemainingByExpiry,
  createSessionClockCache,
  DEFAULT_CAFE_TZ,
  reanchorSessionClockCache,
  SESSION_CLOCK_TICK_MS,
  type SessionClockCache,
  tickSessionClockCache,
} from '@gaming-cafe/contracts';
import { useEffect, useRef, useSyncExternalStore } from 'react';

export interface SessionTimerInput {
  sessionStartTime?: string;
  walletBalanceMinutes?: number;
  timeCreditsConsumed?: number | null;
  deductionProfile?: import('@gaming-cafe/contracts').DeductionProfile | null;
  cafeTimezone?: string;
  expiryDate?: string | null;
}

type Listener = () => void;

let cache: SessionClockCache | null = null;
let displayRemaining: number | null = null;
const listeners = new Set<Listener>();
let intervalId: ReturnType<typeof setInterval> | null = null;

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

function stopInterval() {
  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
}

function syncDisplay(next: number | null) {
  if (displayRemaining === next) return;
  displayRemaining = next;
  emit();
}

function rebuildCache(input: SessionTimerInput | undefined) {
  if (!input?.sessionStartTime || typeof input.walletBalanceMinutes !== 'number') {
    cache = null;
    syncDisplay(null);
    stopInterval();
    return;
  }

  cache = createSessionClockCache(
    input.sessionStartTime,
    input.walletBalanceMinutes,
    input.timeCreditsConsumed ?? 0,
    input.deductionProfile,
    input.cafeTimezone ?? DEFAULT_CAFE_TZ,
    input.expiryDate,
  );
  syncDisplay(cache?.remainingMinutes ?? null);

  stopInterval();
  if (!cache) return;

  intervalId = setInterval(() => {
    if (!cache) return;
    cache = tickSessionClockCache(cache);
    syncDisplay(cache.remainingMinutes);
  }, SESSION_CLOCK_TICK_MS);
}

function reanchor(input: SessionTimerInput) {
  if (!cache || !input.sessionStartTime || typeof input.walletBalanceMinutes !== 'number') {
    rebuildCache(input);
    return;
  }
  cache = reanchorSessionClockCache(
    cache,
    input.walletBalanceMinutes,
    input.timeCreditsConsumed ?? 0,
  );
  syncDisplay(cache.remainingMinutes);
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): number | null {
  return displayRemaining;
}

/** Subscribe to countdown display updates without re-rendering SessionPage. */
export function useSessionTimerDisplay(): number | null {
  return useSyncExternalStore(subscribe, getSnapshot, () => null);
}

/** Current remaining minutes (non-reactive). */
export function getSessionTimerRemainingRef(): number | null {
  return displayRemaining;
}

/**
 * Drives the shared session timer from SessionPage without re-rendering the page each tick.
 * Side-effect callbacks receive the latest remaining minutes once per tick.
 */
export function useSessionTimerController(
  input: SessionTimerInput | undefined,
  onTick?: (remainingMinutes: number | null) => void,
): void {
  const onTickRef = useRef(onTick);
  onTickRef.current = onTick;

  const sessionStartTime = input?.sessionStartTime;
  const walletBalanceMinutes = input?.walletBalanceMinutes;
  const timeCreditsConsumed = input?.timeCreditsConsumed ?? 0;
  const deductionProfile = input?.deductionProfile ?? null;
  const cafeTimezone = input?.cafeTimezone ?? DEFAULT_CAFE_TZ;
  const expiryDate = input?.expiryDate ?? null;

  useEffect(() => {
    rebuildCache(
      sessionStartTime && typeof walletBalanceMinutes === 'number'
        ? {
            sessionStartTime,
            walletBalanceMinutes,
            timeCreditsConsumed,
            deductionProfile,
            cafeTimezone,
            expiryDate,
          }
        : undefined,
    );
    return () => {
      stopInterval();
      cache = null;
      syncDisplay(null);
    };
  }, [
    sessionStartTime,
    walletBalanceMinutes,
    timeCreditsConsumed,
    deductionProfile,
    cafeTimezone,
    expiryDate,
  ]);

  useEffect(() => {
    if (!sessionStartTime || typeof walletBalanceMinutes !== 'number') return;
    reanchor({
      sessionStartTime,
      walletBalanceMinutes,
      timeCreditsConsumed,
      deductionProfile,
      cafeTimezone,
      expiryDate,
    });
  }, [
    sessionStartTime,
    walletBalanceMinutes,
    timeCreditsConsumed,
    deductionProfile,
    cafeTimezone,
    expiryDate,
  ]);

  useEffect(() => {
    if (!onTickRef.current) return;
    return subscribe(() => {
      onTickRef.current?.(displayRemaining);
    });
  }, []);
}

export function capSessionTimerRemaining(
  remainingMinutes: number | null,
  expiryDate: string | null | undefined,
): number | null {
  if (typeof remainingMinutes !== 'number') return null;
  return capRemainingByExpiry(remainingMinutes, expiryDate);
}
