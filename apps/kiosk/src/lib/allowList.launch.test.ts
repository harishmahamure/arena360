import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  allowListPaths,
  formatLaunchArguments,
  type LaunchEntry,
  launcherDisplayName,
  launchViaLabel,
  normalizeLaunchArguments,
  resolveLaunch,
  tokenizeArguments,
} from './allowList';
import { isSoftLaunchEntry, launchEntry, launchEntryForTest, launchErrorMessage } from './launch';
import { detectLaunchProfile, launchAllowed, launchAllowedTest } from './tauriCommands';

vi.mock('./tauriCommands', () => ({
  detectLaunchProfile: vi.fn(),
  launchAllowed: vi.fn(),
  launchAllowedTest: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(detectLaunchProfile).mockResolvedValue(null);
});

const valorantEntry: LaunchEntry = {
  id: '1',
  name: 'Valorant',
  launchMode: 'launcher',
  executablePath: 'C:\\Riot Games\\VALORANT\\live\\VALORANT.exe',
  launchVia: {
    executablePath: 'C:\\Riot Games\\Riot Client\\RiotClientServices.exe',
    arguments: ['--launch-product=valorant', '--launch-patchline=live'],
  },
};

describe('tokenizeArguments', () => {
  it('respects quoted tokens', () => {
    expect(tokenizeArguments('--exec="launch D3"')).toEqual(['--exec=launch D3']);
  });

  it('keeps a detected Battle.net game argument intact when displayed and edited', () => {
    const display = formatLaunchArguments(['--exec=launch D3']);
    expect(display).toBe('"--exec=launch D3"');
    expect(normalizeLaunchArguments(display)).toEqual(['--exec=launch D3']);
  });
});

describe('normalizeLaunchArguments', () => {
  it('accepts legacy string args', () => {
    expect(normalizeLaunchArguments('-applaunch 730')).toEqual(['-applaunch', '730']);
  });

  it('passes through argv arrays', () => {
    expect(normalizeLaunchArguments(['-applaunch', '730'])).toEqual(['-applaunch', '730']);
  });
});

describe('resolveLaunch', () => {
  it('uses launchVia when present', () => {
    expect(resolveLaunch(valorantEntry)).toEqual({
      executablePath: valorantEntry.launchVia?.executablePath,
      arguments: ['--launch-product=valorant', '--launch-patchline=live'],
      entryPath: valorantEntry.executablePath,
    });
  });

  it('falls back to direct launch', () => {
    const direct: LaunchEntry = {
      id: '2',
      name: 'Minecraft',
      executablePath: 'C:\\Games\\minecraft.exe',
      arguments: '--fullscreen',
    };
    expect(resolveLaunch(direct)).toEqual({
      executablePath: direct.executablePath,
      arguments: ['--fullscreen'],
      entryPath: direct.executablePath,
    });
  });

  it('supports launchVia with legacy string arguments', () => {
    const legacy: LaunchEntry = {
      ...valorantEntry,
      launchVia: {
        executablePath: valorantEntry.launchVia?.executablePath ?? '',
        arguments: '--launch-product=valorant --launch-patchline=live',
      },
    };
    expect(resolveLaunch(legacy).arguments).toEqual([
      '--launch-product=valorant',
      '--launch-patchline=live',
    ]);
  });

  it('opens Riot Client for automatic Valorant entries without starting the game', () => {
    expect(resolveLaunch({ ...valorantEntry, launchMode: 'auto' })).toEqual({
      executablePath: valorantEntry.launchVia?.executablePath,
      arguments: undefined,
      entryPath: valorantEntry.executablePath,
    });
  });

  it('opens Riot first for a legacy scan profile without an explicit launch mode', () => {
    expect(
      resolveLaunch({ ...valorantEntry, launchMode: undefined, launchViaAuto: true }).arguments,
    ).toBeUndefined();
  });

  it('ignores stored launcher profiles when direct launch is explicitly selected', () => {
    expect(
      resolveLaunch({ ...valorantEntry, launchMode: 'direct', arguments: '--fullscreen' }),
    ).toEqual({
      executablePath: valorantEntry.executablePath,
      arguments: ['--fullscreen'],
      entryPath: valorantEntry.executablePath,
    });
  });

  it('opens any platform client without game arguments in launcher-only mode', () => {
    const entry: LaunchEntry = {
      ...valorantEntry,
      launchMode: 'launcher-only',
      launchVia: { executablePath: 'C:\\Steam\\steam.exe', arguments: '-applaunch 730' },
    };
    expect(resolveLaunch(entry).arguments).toBeUndefined();
  });
});

describe('allowListPaths', () => {
  it('includes launcher paths from launchVia entries', () => {
    const paths = allowListPaths([valorantEntry]);
    expect(paths).toContain(valorantEntry.executablePath);
    expect(paths).toContain(valorantEntry.launchVia?.executablePath);
  });
});

describe('launchViaLabel', () => {
  it('labels riot and steam launchers', () => {
    expect(launchViaLabel(valorantEntry)).toBe('Riot Client');
    expect(
      launchViaLabel({
        ...valorantEntry,
        launchVia: {
          executablePath: 'C:\\Steam\\steam.exe',
          arguments: ['-applaunch', '730'],
        },
      }),
    ).toBe('Steam');
  });
});

describe('launcherDisplayName', () => {
  it('labels major platform launchers', () => {
    expect(launcherDisplayName('C:\\Program Files (x86)\\Battle.net\\Battle.net.exe')).toBe(
      'Battle.net',
    );
    expect(
      launcherDisplayName(
        'C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EALaunchHelper.exe',
      ),
    ).toBe('EA App');
    expect(launcherDisplayName('C:\\Program Files (x86)\\GOG Galaxy\\GalaxyClient.exe')).toBe(
      'GOG Galaxy',
    );
  });
});

describe('launchEntry validation', () => {
  it('rejects empty launcher path', async () => {
    const entry: LaunchEntry = {
      id: '1',
      name: 'Test',
      executablePath: 'C:\\Games\\game.exe',
      launchVia: { executablePath: '  ', arguments: ['-applaunch', '1'] },
    };
    await expect(launchEntry(entry, [entry])).rejects.toThrow(/Launcher path is required/);
  });

  it('rejects empty allow-list', async () => {
    const entry: LaunchEntry = {
      id: '1',
      name: 'Test',
      executablePath: 'C:\\Games\\game.exe',
    };
    await expect(launchEntry(entry, [])).rejects.toThrow(/No applications are configured/);
  });

  it('detects a legacy Valorant path and launches only Riot in player and setup flows', async () => {
    const entry: LaunchEntry = {
      ...valorantEntry,
      launchMode: 'auto',
      launchVia: undefined,
      runAsAdmin: true,
    };
    vi.mocked(detectLaunchProfile).mockResolvedValue(valorantEntry.launchVia ?? null);
    await launchEntry(entry, [entry]);
    await launchEntryForTest(entry, [entry]);
    expect(launchAllowed).toHaveBeenCalledExactlyOnceWith(
      valorantEntry.launchVia?.executablePath,
      [entry.executablePath, valorantEntry.launchVia?.executablePath],
      undefined,
      false,
      true,
    );
    expect(launchAllowedTest).toHaveBeenCalledExactlyOnceWith(
      valorantEntry.launchVia?.executablePath,
      [entry.executablePath, valorantEntry.launchVia?.executablePath],
      undefined,
      true,
    );
  });

  it('does not start the game exe if Riot detection fails', async () => {
    const entry: LaunchEntry = { ...valorantEntry, launchMode: 'auto', launchVia: undefined };
    vi.mocked(detectLaunchProfile).mockRejectedValue('Riot Client was not found');
    await expect(launchEntry(entry, [entry])).rejects.toBe('Riot Client was not found');
    expect(launchAllowed).not.toHaveBeenCalled();
  });

  it('does not detect a launcher when direct launch was chosen', async () => {
    const entry: LaunchEntry = { ...valorantEntry, launchMode: 'direct', launchVia: undefined };
    await launchEntry(entry, [entry]);
    expect(detectLaunchProfile).not.toHaveBeenCalled();
    expect(launchAllowed).toHaveBeenCalledWith(
      entry.executablePath,
      [entry.executablePath],
      undefined,
      false,
      undefined,
    );
  });

  it('does not launch an unconfigured game or launcher', async () => {
    await expect(
      launchEntry(valorantEntry, [
        { ...valorantEntry, executablePath: 'C:\\Other.exe', launchVia: undefined },
      ]),
    ).rejects.toThrow('Executable not in allow-list');
    await expect(
      launchEntry(valorantEntry, [{ ...valorantEntry, launchVia: undefined }]),
    ).rejects.toThrow('Executable not in allow-list');
    expect(launchAllowed).not.toHaveBeenCalled();
    expect(detectLaunchProfile).not.toHaveBeenCalled();
  });

  it('allows a launcher without arguments and rejects a missing manual launcher', async () => {
    const entry: LaunchEntry = {
      ...valorantEntry,
      launchVia: { executablePath: 'C:\\Launcher.exe', arguments: [] },
    };
    await launchEntry(entry, [entry]);
    expect(launchAllowed).toHaveBeenCalledWith(
      'C:\\Launcher.exe',
      expect.any(Array),
      undefined,
      false,
      undefined,
    );
    await expect(launchEntry({ ...entry, launchVia: undefined }, [entry])).rejects.toThrow(
      'Launcher path is required',
    );
  });
});

describe('isSoftLaunchEntry', () => {
  it('marks util and launcher categories as soft launch', () => {
    expect(
      isSoftLaunchEntry({ id: '1', name: 'Steam', executablePath: 'x', category: 'launcher' }),
    ).toBe(true);
    expect(
      isSoftLaunchEntry({ id: '2', name: 'Discord', executablePath: 'x', category: 'util' }),
    ).toBe(true);
    expect(
      isSoftLaunchEntry({ id: '3', name: 'Game', executablePath: 'x', category: 'game' }),
    ).toBe(false);
  });
});

describe('launchEntryForTest', () => {
  it('rejects missing executables before invoke', async () => {
    const entry: LaunchEntry = {
      id: '1',
      name: 'Missing',
      executablePath: 'C:\\Missing\\app.exe',
      present: false,
    };
    await expect(launchEntryForTest(entry, [entry])).rejects.toThrow(/Executable not found/);
  });
});

describe('launchErrorMessage', () => {
  it('uses Error.message', () => {
    expect(launchErrorMessage(new Error('Executable not in allow-list'), 'fallback')).toBe(
      'Executable not in allow-list',
    );
  });

  it('uses string rejections from Tauri', () => {
    expect(launchErrorMessage('Session cleanup in progress', 'Could not launch GTA')).toBe(
      'Session cleanup in progress',
    );
  });

  it('falls back when message is empty', () => {
    expect(launchErrorMessage('   ', 'Could not launch GTA')).toBe('Could not launch GTA');
  });
});
