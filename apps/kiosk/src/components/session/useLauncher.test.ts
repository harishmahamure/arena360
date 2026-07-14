import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { LaunchEntry } from '../../lib/allowList';
import { appendKioskLog } from '../../lib/bootDiagnostics';
import { useLauncher } from './useLauncher';

vi.mock('../../lib/bootDiagnostics', () => ({
  appendKioskLog: vi.fn(),
}));

vi.mock('../../lib/launch', () => ({
  installedEntries: () => [],
  launchEntry: vi.fn(),
  launchErrorMessage: (_e: unknown, fallback: string) => fallback,
}));

const entry: LaunchEntry = {
  id: 'game-1',
  name: 'Test Game',
  executablePath: 'C:\\Games\\test.exe',
};

describe('useLauncher', () => {
  it('logs launch failures before showing toast', async () => {
    const { launchEntry } = await import('../../lib/launch');
    vi.mocked(launchEntry).mockRejectedValue(new Error('spawn failed'));
    const onError = vi.fn();

    const { result } = renderHook(() => useLauncher(false, onError));

    await act(async () => {
      await result.current.launch(entry);
    });

    expect(appendKioskLog).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('launch failed entry=game-1 name=Test Game'),
    );
    expect(onError).toHaveBeenCalledWith('Could not launch Test Game');
  });
});
