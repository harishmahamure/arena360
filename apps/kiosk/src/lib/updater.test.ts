import { beforeEach, describe, expect, it, vi } from 'vitest';
import { appendKioskLog } from './bootDiagnostics';
import { checkForUpdateWhenIdle } from './updater';

vi.mock('./bootDiagnostics', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./bootDiagnostics')>();
  return {
    ...actual,
    appendKioskLog: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

const check = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: () => check(),
}));

describe('isIdleUpdatePhase', () => {
  it('returns true for register, setup, login, and create-account phases', async () => {
    const { isIdleUpdatePhase } = await import('./updater');
    expect(isIdleUpdatePhase('register')).toBe(true);
    expect(isIdleUpdatePhase('setup')).toBe(true);
    expect(isIdleUpdatePhase('login')).toBe(true);
    expect(isIdleUpdatePhase('create-account')).toBe(true);
    expect(isIdleUpdatePhase('create-account-success')).toBe(true);
  });

  it('returns false for loading, session, and already-in-session', async () => {
    const { isIdleUpdatePhase } = await import('./updater');
    expect(isIdleUpdatePhase('loading')).toBe(false);
    expect(isIdleUpdatePhase('session')).toBe(false);
    expect(isIdleUpdatePhase('already-in-session')).toBe(false);
  });

  it('marks non-idle phases for update cancellation', async () => {
    const { isIdleUpdatePhase, setUpdatePhase } = await import('./updater');
    setUpdatePhase('login');
    setUpdatePhase('session');
    expect(isIdleUpdatePhase('session')).toBe(false);
  });
});

describe('checkForUpdateWhenIdle', () => {
  beforeEach(() => {
    vi.mocked(appendKioskLog).mockReset();
    check.mockReset();
  });

  it('logs skipped update checks to kiosk.log', async () => {
    check.mockRejectedValue(new Error('no endpoint'));
    await checkForUpdateWhenIdle('login');
    expect(appendKioskLog).toHaveBeenCalledWith(
      'warn',
      expect.stringContaining('[updater] update check skipped:'),
    );
  });
});
