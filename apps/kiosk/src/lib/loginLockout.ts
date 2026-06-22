/**
 * Client-side brute-force lockout for the player login form: after
 * MAX_FAILURES failed attempts within WINDOW_MS, logins are blocked until the
 * window elapses. This is a UX guardrail only — the backend remains the
 * authoritative rate limiter.
 */

const STORAGE_KEY = 'gaming-cafe.kiosk.login_failures';
export const MAX_FAILURES = 5;
export const WINDOW_MS = 15 * 60 * 1000;

function readTimestamps(now: number): number[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.filter((t): t is number => typeof t === 'number' && now - t < WINDOW_MS);
  } catch {
    return [];
  }
}

function write(timestamps: number[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(timestamps));
  } catch {
    // non-fatal
  }
}

export interface LockoutState {
  locked: boolean;
  /** Epoch ms when the lock lifts (only meaningful when locked). */
  retryAt: number;
  remainingAttempts: number;
}

export function getLockout(now: number = Date.now()): LockoutState {
  const fails = readTimestamps(now);
  const locked = fails.length >= MAX_FAILURES;
  const oldest = fails[0] ?? now;
  return {
    locked,
    retryAt: oldest + WINDOW_MS,
    remainingAttempts: Math.max(0, MAX_FAILURES - fails.length),
  };
}

export function recordFailure(now: number = Date.now()): LockoutState {
  const fails = readTimestamps(now);
  fails.push(now);
  write(fails);
  return getLockout(now);
}

export function clearFailures(): void {
  write([]);
}

/** Staff reset from the login screen Clear sign-in lock button. */
export function resetLoginLockoutByStaff(): LockoutState {
  clearFailures();
  return getLockout();
}
