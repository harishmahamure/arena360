import { describe, expect, it } from 'vitest';
import { exportAllowListJson, importAllowListJson, loadLaunchEntries } from './allowList';

describe('allowList import/export', () => {
  it('round-trips entries through JSON export/import', () => {
    localStorage.setItem(
      'gaming-cafe.kiosk.launch_entries',
      JSON.stringify([
        {
          id: 'a',
          name: 'Test Game',
          executablePath: 'C:\\Games\\test.exe',
          category: 'game',
        },
      ]),
    );

    const exported = exportAllowListJson();
    localStorage.clear();
    const imported = importAllowListJson(exported);

    expect(imported).toHaveLength(1);
    expect(imported[0]?.name).toBe('Test Game');
    expect(loadLaunchEntries()).toHaveLength(1);
  });

  it('rejects invalid import payloads', () => {
    expect(() => importAllowListJson('[]')).toThrow(/No valid allow-list entries/);
  });
});
