import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { loadLaunchEntries, mergeScanCandidates, saveLaunchEntries } from './allowList';
import type { ScanCandidate } from './tauriCommands';

const STORAGE_KEY = 'gaming-cafe.kiosk.launch_entries';

const riotLauncher = 'C:\\Riot Games\\Riot Client\\RiotClientServices.exe';
const valorantExe = 'C:\\Riot Games\\VALORANT\\live\\VALORANT.exe';

const valorantCandidate: ScanCandidate = {
  name: 'Valorant',
  executablePath: valorantExe,
  source: 'known',
  present: true,
  launchVia: {
    executablePath: riotLauncher,
    arguments: ['--launch-product=valorant', '--launch-patchline=live'],
  },
};

const registryNoise: ScanCandidate = {
  name: 'Some Registry App',
  executablePath: 'C:\\Apps\\noise.exe',
  source: 'registry',
  present: true,
};

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  localStorage.clear();
});

describe('mergeScanCandidates', () => {
  it('merges trusted game with launchVia and auto-adds launcher', () => {
    const result = mergeScanCandidates([valorantCandidate]);

    expect(result.added).toBe(2);
    expect(result.launchersAdded).toBe(1);
    expect(result.updated).toBe(0);

    const entries = loadLaunchEntries();
    expect(entries).toHaveLength(2);
    expect(entries.find((e) => e.executablePath === valorantExe)?.launchVia).toEqual(
      valorantCandidate.launchVia,
    );
    expect(entries.find((e) => e.executablePath === riotLauncher)?.category).toBe('launcher');
  });

  it('skips registry and drive-scan sources', () => {
    const driveScan: ScanCandidate = {
      name: 'Random Game',
      executablePath: 'D:\\Games\\random.exe',
      source: 'drive-scan',
      present: true,
    };

    const result = mergeScanCandidates([registryNoise, driveScan, valorantCandidate]);

    expect(loadLaunchEntries().some((e) => e.executablePath === registryNoise.executablePath)).toBe(
      false,
    );
    expect(loadLaunchEntries().some((e) => e.executablePath === driveScan.executablePath)).toBe(
      false,
    );
    expect(result.added).toBe(2);
  });

  it('dedupes on second merge', () => {
    mergeScanCandidates([valorantCandidate]);
    const second = mergeScanCandidates([valorantCandidate]);

    expect(second.added).toBe(0);
    expect(second.updated).toBe(0);
    expect(loadLaunchEntries()).toHaveLength(2);
  });

  it('upgrades existing Valorant row with launchVia from scan', () => {
    saveLaunchEntries([
      {
        id: 'existing',
        name: 'VALORANT',
        executablePath: valorantExe,
        category: 'game',
        present: true,
      },
    ]);

    const result = mergeScanCandidates([valorantCandidate]);

    expect(result.added).toBe(1);
    expect(result.launchersAdded).toBe(1);
    expect(result.updated).toBe(1);
    expect(loadLaunchEntries().find((e) => e.id === 'existing')?.launchVia).toEqual(
      valorantCandidate.launchVia,
    );
  });

  it('skips absent candidates', () => {
    const result = mergeScanCandidates([{ ...valorantCandidate, present: false }]);

    expect(result.added).toBe(0);
    expect(localStorage.getItem(STORAGE_KEY)).toBe('[]');
  });

  it('upgrades existing direct-launch Steam entry on re-scan', () => {
    const cs2Exe = 'C:\\Steam\\steamapps\\common\\Counter-Strike Global Offensive\\cs2.exe';
    const steamLauncher = 'C:\\Steam\\steam.exe';
    saveLaunchEntries([
      {
        id: 'steam-direct',
        name: 'Counter-Strike 2',
        executablePath: cs2Exe,
        category: 'game',
        present: true,
      },
    ]);

    const steamCandidate: ScanCandidate = {
      name: 'Counter-Strike 2',
      executablePath: cs2Exe,
      source: 'steam',
      present: true,
      launchVia: {
        executablePath: steamLauncher,
        arguments: ['-applaunch', '730'],
      },
    };

    const result = mergeScanCandidates([steamCandidate]);

    expect(result.updated).toBe(1);
    expect(loadLaunchEntries().find((e) => e.id === 'steam-direct')?.launchVia).toEqual({
      executablePath: steamLauncher,
      arguments: ['-applaunch', '730'],
    });
  });

  it('overwrites operator-edited launchVia when trusted scan provides a profile', () => {
    saveLaunchEntries([
      {
        id: 'existing',
        name: 'VALORANT',
        executablePath: valorantExe,
        category: 'game',
        present: true,
        launchVia: {
          executablePath: riotLauncher,
          arguments: ['--launch-product=valorant', '--launch-patchline=pbe'],
        },
      },
    ]);

    const result = mergeScanCandidates([valorantCandidate]);

    expect(result.updated).toBe(1);
    expect(loadLaunchEntries().find((e) => e.id === 'existing')?.launchVia).toEqual(
      valorantCandidate.launchVia,
    );
  });
});
