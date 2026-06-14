import { describe, expect, it } from 'vitest';
import { isIdleUpdatePhase } from './updater';

describe('isIdleUpdatePhase', () => {
  it('returns true for register, setup, and login', () => {
    expect(isIdleUpdatePhase('register')).toBe(true);
    expect(isIdleUpdatePhase('setup')).toBe(true);
    expect(isIdleUpdatePhase('login')).toBe(true);
  });

  it('returns false for loading, session, and already-in-session', () => {
    expect(isIdleUpdatePhase('loading')).toBe(false);
    expect(isIdleUpdatePhase('session')).toBe(false);
    expect(isIdleUpdatePhase('already-in-session')).toBe(false);
  });
});
