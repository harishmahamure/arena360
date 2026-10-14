import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { type LaunchEntry, loadLaunchEntries, saveLaunchEntries } from '../lib/allowList';
import {
  detectLaunchProfile,
  launchAllowedTest,
  pickExecutable,
  scanInstalledSoftware,
} from '../lib/tauriCommands';
import { AllowListEditor } from './AllowListEditor';

vi.mock('@gaming-cafe/ui/primitives', () => ({ IconFallback: () => null }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('../lib/gallery', () => ({ refreshGallery: vi.fn().mockResolvedValue([]) }));
vi.mock('../lib/gameBoost', () => ({
  applyGameBoostSettings: vi.fn(),
  loadGameBoostSettings: () => ({ enabled: false, aggressive: false }),
  syncGameBoostToNative: vi.fn(),
}));
vi.mock('../lib/tauriCommands', () => ({
  detectLaunchProfile: vi.fn(),
  scanInstalledSoftware: vi.fn(),
  launchAllowedTest: vi.fn(),
  launchAllowed: vi.fn(),
  pickExecutable: vi.fn(),
}));
vi.mock('./GalleryPicker', () => ({ GalleryPicker: () => null }));

const entry: LaunchEntry = {
  id: 'valorant',
  name: 'VALORANT',
  category: 'game',
  present: true,
  executablePath: 'D:\\Games\\VALORANT\\live\\VALORANT.exe',
};
const launchVia = {
  executablePath: 'C:\\Riot Games\\Riot Client\\RiotClientServices.exe',
  arguments: ['--launch-product=valorant', '--launch-patchline=live'],
};

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  vi.mocked(scanInstalledSoftware).mockResolvedValue([]);
  vi.mocked(detectLaunchProfile).mockResolvedValue(launchVia);
  saveLaunchEntries([entry]);
});

async function selectValorant() {
  render(<AllowListEditor />);
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Auto search installed apps' })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole('button', { name: 'VALORANT' }));
}

describe('Setup launch controls', () => {
  it('auto-searches existing stations and tests Valorant through Riot Client only', async () => {
    await selectValorant();
    expect(scanInstalledSoftware).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Auto-detect launcher' }));
    await waitFor(() => expect(loadLaunchEntries()[0]?.launchVia).toEqual(launchVia));
    expect(screen.getByRole('combobox', { name: /Launch option/ })).toHaveValue('auto');
    fireEvent.click(screen.getByRole('button', { name: 'Test launch' }));
    await waitFor(() =>
      expect(launchAllowedTest).toHaveBeenCalledWith(
        launchVia.executablePath,
        [entry.executablePath, launchVia.executablePath],
        undefined,
        undefined,
      ),
    );
  });

  it('saves direct launch and administrator settings across a re-scan', async () => {
    await selectValorant();
    fireEvent.change(screen.getByRole('combobox', { name: /Launch option/ }), {
      target: { value: 'direct' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: /Run as Windows administrator/ }));
    vi.mocked(scanInstalledSoftware).mockResolvedValue([
      { ...entry, source: 'known', present: true, launchVia, launchProfileFromScan: true },
    ]);
    fireEvent.click(screen.getByRole('button', { name: 'Auto search installed apps' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Auto search installed apps' })).toBeEnabled(),
    );
    expect(loadLaunchEntries().find((e) => e.id === entry.id)).toMatchObject({
      launchMode: 'direct',
      runAsAdmin: true,
    });
    fireEvent.click(screen.getByRole('button', { name: 'Test launch' }));
    await waitFor(() =>
      expect(launchAllowedTest).toHaveBeenCalledWith(
        entry.executablePath,
        expect.any(Array),
        undefined,
        true,
      ),
    );
    expect(detectLaunchProfile).not.toHaveBeenCalled();
  });

  it('surfaces detection failures and keeps the previous launcher configuration', async () => {
    saveLaunchEntries([{ ...entry, launchVia, launchMode: 'launcher-only' }]);
    render(<AllowListEditor />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Auto search installed apps' })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole('button', { name: 'VALORANT' }));
    vi.mocked(detectLaunchProfile).mockRejectedValue('Riot Client was not found');
    fireEvent.click(screen.getByRole('button', { name: 'Auto-detect launcher' }));
    expect(await screen.findByText('Riot Client was not found')).toBeInTheDocument();
    expect(loadLaunchEntries()[0]?.launchMode).toBe('launcher-only');
    expect(loadLaunchEntries()[0]?.launchVia).toEqual(launchVia);
  });

  it('preserves a browsed launcher and quoted arguments across scans', async () => {
    saveLaunchEntries([{ ...entry, launchVia, launchViaAuto: true }]);
    await selectValorant();
    vi.mocked(pickExecutable).mockResolvedValue('E:\\Riot Client\\RiotClientServices.exe');
    fireEvent.click(
      within(screen.getByText('Launcher executable')).getByRole('button', { name: 'Browse…' }),
    );
    await waitFor(() => expect(loadLaunchEntries()[0]?.launchMode).toBe('launcher'));
    fireEvent.change(screen.getByRole('textbox', { name: 'Launcher arguments' }), {
      target: { value: '--config="two words"' },
    });
    expect(screen.getByRole('textbox', { name: 'Launcher arguments' })).toHaveValue(
      '--config="two words"',
    );
    vi.mocked(scanInstalledSoftware).mockResolvedValue([
      { ...entry, source: 'known', present: true, launchVia, launchProfileFromScan: true },
    ]);
    fireEvent.click(screen.getByRole('button', { name: 'Auto search installed apps' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Auto search installed apps' })).toBeEnabled(),
    );
    expect(loadLaunchEntries().find((e) => e.id === entry.id)?.launchVia).toEqual({
      executablePath: 'E:\\Riot Client\\RiotClientServices.exe',
      arguments: '--config="two words"',
    });
  });
});
