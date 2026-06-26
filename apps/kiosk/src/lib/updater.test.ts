import { describe, expect, it, vi } from 'vitest';
import { isIdleUpdatePhase, setUpdatePhase } from './updater';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(),
}));

describe('isIdleUpdatePhase', () => {
  it('returns true for register, setup, login, and create-account phases', () => {
    expect(isIdleUpdatePhase('register')).toBe(true);
    expect(isIdleUpdatePhase('setup')).toBe(true);
    expect(isIdleUpdatePhase('login')).toBe(true);
    expect(isIdleUpdatePhase('create-account')).toBe(true);
    expect(isIdleUpdatePhase('create-account-success')).toBe(true);
  });

  it('returns false for loading, session, and already-in-session', () => {
    expect(isIdleUpdatePhase('loading')).toBe(false);
    expect(isIdleUpdatePhase('session')).toBe(false);
    expect(isIdleUpdatePhase('already-in-session')).toBe(false);
  });

  it('marks non-idle phases for update cancellation', () => {
    setUpdatePhase('login');
    setUpdatePhase('session');
    expect(isIdleUpdatePhase('session')).toBe(false);
  });
});
