import { describe, expect, it } from 'vitest';
import {
  allowListPaths,
  type LaunchEntry,
  launcherDisplayName,
  launchViaLabel,
  normalizeLaunchArguments,
  resolveLaunch,
  tokenizeArguments,
} from './allowList';
import { launchEntry, launchErrorMessage } from './launch';

const valorantEntry: LaunchEntry = {
  id: '1',
  name: 'Valorant',
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
