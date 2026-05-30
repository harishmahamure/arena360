import { beforeEach, describe, expect, it } from 'vitest';
import { clearFailures, getLockout, MAX_FAILURES, recordFailure } from './loginLockout';

describe('loginLockout', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('starts unlocked with full attempts', () => {
    const state = getLockout();
    expect(state.locked).toBe(false);
    expect(state.remainingAttempts).toBe(MAX_FAILURES);
  });

  it('locks after MAX_FAILURES failures', () => {
    let state = getLockout();
    for (let i = 0; i < MAX_FAILURES; i++) {
      state = recordFailure();
    }
    expect(state.locked).toBe(true);
    expect(state.remainingAttempts).toBe(0);
  });

  it('decrements remaining attempts as failures accrue', () => {
    recordFailure();
    expect(getLockout().remainingAttempts).toBe(MAX_FAILURES - 1);
  });

  it('clears failures on success', () => {
    recordFailure();
    recordFailure();
    clearFailures();
    expect(getLockout().remainingAttempts).toBe(MAX_FAILURES);
  });

  it('drops failures older than the window', () => {
    const now = 1_000_000_000_000;
    for (let i = 0; i < MAX_FAILURES; i++) recordFailure(now);
    // 16 minutes later the window has elapsed.
    const later = now + 16 * 60 * 1000;
    expect(getLockout(later).locked).toBe(false);
  });
});
