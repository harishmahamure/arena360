import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { appendKioskLog } from './bootDiagnostics';
import { checkForUpdateWhenIdle, setUpdatePhase } from './updater';

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
    vi.mocked(invoke).mockReset();
    vi.mocked(relaunch).mockReset();
    setUpdatePhase('login');
  });

  it('restores recovery when installation fails after pausing it', async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error('download failed'));
    check.mockResolvedValue({ downloadAndInstall });
    await checkForUpdateWhenIdle('login');
    expect(invoke).toHaveBeenNthCalledWith(1, 'prepare_for_update');
    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenNthCalledWith(2, 'resume_kiosk_recovery');
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('restores recovery when a player session cancels the installer handoff', async () => {
    const downloadAndInstall = vi.fn();
    check.mockResolvedValue({ downloadAndInstall });
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'prepare_for_update') setUpdatePhase('session');
      return undefined;
    });
    await checkForUpdateWhenIdle('login');
    expect(downloadAndInstall).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenLastCalledWith('resume_kiosk_recovery');
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('leaves recovery alone when no update is available', async () => {
    check.mockResolvedValue(null);
    await checkForUpdateWhenIdle('login');
    expect(invoke).not.toHaveBeenCalled();
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
