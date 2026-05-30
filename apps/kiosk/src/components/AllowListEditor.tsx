import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import {
  addLaunchEntry,
  allowListPaths,
  type LaunchEntry,
  loadLaunchEntries,
  removeLaunchEntry,
} from '../lib/allowList';
import { launchAllowed, type ScanCandidate, scanInstalledSoftware } from '../lib/tauriCommands';

interface ScanProgress {
  scanned: number;
  total: number;
}

export function AllowListEditor() {
  const [entries, setEntries] = useState<LaunchEntry[]>(() => loadLaunchEntries());
  const [candidates, setCandidates] = useState<ScanCandidate[]>([]);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [manualName, setManualName] = useState('');
  const [manualPath, setManualPath] = useState('');
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<ScanProgress>('scan-progress', (event) => {
      setProgress(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const runScan = useCallback(async () => {
    setScanning(true);
    setProgress(null);
    setStatus(null);
    try {
      const found = await scanInstalledSoftware();
      setCandidates(found);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : 'Scan failed');
    } finally {
      setScanning(false);
    }
  }, []);

  function addCandidate(c: ScanCandidate) {
    setEntries(
      addLaunchEntry({ name: c.name, executablePath: c.executablePath, present: c.present }),
    );
  }

  function addManual(e: React.FormEvent) {
    e.preventDefault();
    if (!manualName.trim() || !manualPath.trim()) return;
    setEntries(addLaunchEntry({ name: manualName.trim(), executablePath: manualPath.trim() }));
    setManualName('');
    setManualPath('');
  }

  function remove(id: string) {
    setEntries(removeLaunchEntry(id));
  }

  async function testLaunch(entry: LaunchEntry) {
    setStatus(null);
    try {
      const result = await launchAllowed(entry.executablePath, allowListPaths(entries));
      setStatus(`Launched ${entry.name} (pid ${result.pid})`);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : `Could not launch ${entry.name}`);
    }
  }

  const alreadyAdded = (path: string) =>
    entries.some((e) => e.executablePath.toLowerCase() === path.toLowerCase());

  return (
    <div className="allow-list-editor">
      <div className="allow-list-section">
        <div className="allow-list-header">
          <h2>Launch allow-list ({entries.length})</h2>
        </div>
        {entries.length === 0 ? (
          <p className="hint">No applications added yet. Scan or add one below.</p>
        ) : (
          <ul className="allow-list">
            {entries.map((entry) => (
              <li key={entry.id} className={entry.present === false ? 'missing' : undefined}>
                <span className="allow-list-name">{entry.name}</span>
                <span className="allow-list-path">{entry.executablePath}</span>
                <span className="allow-list-actions">
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void testLaunch(entry)}
                  >
                    Test
                  </button>
                  <button type="button" className="danger" onClick={() => remove(entry.id)}>
                    Remove
                  </button>
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="allow-list-section">
        <div className="allow-list-header">
          <h2>Detected software</h2>
          <button type="button" onClick={() => void runScan()} disabled={scanning}>
            {scanning ? 'Scanning…' : 'Scan installed software'}
          </button>
        </div>
        {scanning && progress ? <progress value={progress.scanned} max={progress.total} /> : null}
        {candidates.length > 0 ? (
          <ul className="allow-list">
            {candidates.map((c) => (
              <li key={c.executablePath} className={c.present ? undefined : 'missing'}>
                <span className="allow-list-name">{c.name}</span>
                <span className="allow-list-path">{c.executablePath}</span>
                <span className="allow-list-actions">
                  <button
                    type="button"
                    className="secondary"
                    disabled={alreadyAdded(c.executablePath)}
                    onClick={() => addCandidate(c)}
                  >
                    {alreadyAdded(c.executablePath) ? 'Added' : 'Add'}
                  </button>
                </span>
              </li>
            ))}
          </ul>
        ) : null}
      </div>

      <div className="allow-list-section">
        <h2>Add manually</h2>
        <form onSubmit={addManual} className="allow-list-manual">
          <input
            value={manualName}
            onChange={(e) => setManualName(e.target.value)}
            placeholder="Display name"
          />
          <input
            value={manualPath}
            onChange={(e) => setManualPath(e.target.value)}
            placeholder="C:\\Path\\To\\game.exe"
          />
          <button type="submit">Add</button>
        </form>
      </div>

      {status ? <p className="meta">{status}</p> : null}
    </div>
  );
}
