import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppPhase } from '../context/KioskProvider';
import { saveLaunchEntries } from '../lib/allowList';
import { setSignedOutCleanupEnabled } from '../lib/tauriCommands';
import { signedOutCleanupEnabled, useSignedOutCleanup } from './useSignedOutCleanup';

vi.mock('../lib/tauriCommands', () => ({
  setSignedOutCleanupEnabled: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('../lib/bootDiagnostics', () => ({ appendKioskLog: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
});

describe('signed-out cleanup eligibility', () => {
  it.each<AppPhase>([
    'login',
    'register',
    'create-account',
    'create-account-success',
    'already-in-session',
  ])('cleans a confirmed signed-out %s screen', (phase) => {
    expect(signedOutCleanupEnabled(phase, false, false)).toBe(true);
    expect(signedOutCleanupEnabled(phase, true, false)).toBe(false);
    expect(signedOutCleanupEnabled(phase, false, true)).toBe(false);
  });
  it.each<AppPhase>([
    'loading',
    'boot-error',
    'setup',
    'session',
  ])('preserves apps during %s, even without a session object yet', (phase) => {
    expect(signedOutCleanupEnabled(phase, false, false)).toBe(false);
  });
});

describe('native cleanup state synchronization', () => {
  it('enables only after restoration finishes, supplying game and launcher paths', async () => {
    saveLaunchEntries([
      {
        id: 'v',
        name: 'Valorant',
        executablePath: 'D:\\Games\\VALORANT.exe',
        launchVia: { executablePath: 'C:\\RiotClientServices.exe', arguments: [] },
      },
    ]);
    const { rerender, unmount } = renderHook(
      ({ phase, hasSession, signingIn }) => useSignedOutCleanup(phase, hasSession, signingIn),
      {
        initialProps: { phase: 'loading' as AppPhase, hasSession: false, signingIn: false },
      },
    );
    await waitFor(() => expect(setSignedOutCleanupEnabled).toHaveBeenLastCalledWith(false, []));
    rerender({ phase: 'login', hasSession: false, signingIn: false });
    await waitFor(() =>
      expect(setSignedOutCleanupEnabled).toHaveBeenLastCalledWith(true, [
        'D:\\Games\\VALORANT.exe',
        'C:\\RiotClientServices.exe',
      ]),
    );
    rerender({ phase: 'login', hasSession: false, signingIn: true });
    await waitFor(() => expect(setSignedOutCleanupEnabled).toHaveBeenLastCalledWith(false, []));
    rerender({ phase: 'session', hasSession: true, signingIn: false });
    expect(setSignedOutCleanupEnabled).toHaveBeenLastCalledWith(false, []);
    unmount();
    expect(setSignedOutCleanupEnabled).toHaveBeenLastCalledWith(false);
  });

  it('does not enable cleanup for a restored session during a network failure', () => {
    const { rerender } = renderHook(
      ({ phase, hasSession }) => useSignedOutCleanup(phase, hasSession, false),
      {
        initialProps: { phase: 'loading' as AppPhase, hasSession: false },
      },
    );
    rerender({ phase: 'session', hasSession: true });
    expect(vi.mocked(setSignedOutCleanupEnabled).mock.calls.some(([enabled]) => enabled)).toBe(
      false,
    );
    rerender({ phase: 'login', hasSession: true }); // a session awaiting reauthentication
    expect(vi.mocked(setSignedOutCleanupEnabled).mock.calls.some(([enabled]) => enabled)).toBe(
      false,
    );
  });
});
