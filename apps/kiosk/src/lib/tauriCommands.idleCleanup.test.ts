import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setSignedOutCleanupEnabled } from './tauriCommands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), convertFileSrc: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('./bootDiagnostics', () => ({ appendKioskLog: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe('native signed-out cleanup commands', () => {
  it('delivers enable before pause even when the first invocation is delayed', async () => {
    let finishEnable = () => {};
    vi.mocked(invoke)
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishEnable = resolve;
          }),
      )
      .mockResolvedValue(undefined);
    const enable = setSignedOutCleanupEnabled(true, ['C:\\Games\\game.exe']);
    const pause = setSignedOutCleanupEnabled(false);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    expect(invoke).toHaveBeenLastCalledWith('set_signed_out_cleanup_enabled_cmd', {
      enabled: true,
      appPaths: ['C:\\Games\\game.exe'],
    });
    finishEnable();
    await Promise.all([enable, pause]);
    expect(invoke).toHaveBeenLastCalledWith('set_signed_out_cleanup_enabled_cmd', {
      enabled: false,
      appPaths: [],
    });
  });

  it('still delivers cancellation if an earlier state update failed', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('temporary IPC failure').mockResolvedValue(undefined);
    const enable = setSignedOutCleanupEnabled(true);
    const pause = setSignedOutCleanupEnabled(false);
    await expect(enable).rejects.toBe('temporary IPC failure');
    await expect(pause).resolves.toBeUndefined();
    expect(invoke).toHaveBeenLastCalledWith('set_signed_out_cleanup_enabled_cmd', {
      enabled: false,
      appPaths: [],
    });
  });
});
