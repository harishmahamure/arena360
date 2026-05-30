import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import {
  addLaunchEntry,
  allowListPaths,
  categorizeByName,
  entryCategory,
  type LaunchCategory,
  type LaunchEntry,
  loadLaunchEntries,
  removeLaunchEntry,
  setEntryCategory,
} from '../lib/allowList';
import {
  launchAllowed,
  pickExecutable,
  type ScanCandidate,
  scanInstalledSoftware,
} from '../lib/tauriCommands';
import { IconFallback } from './IconFallback';

interface ScanProgress {
  scanned: number;
  total: number;
}

const CATEGORY_OPTIONS: { value: LaunchCategory; label: string }[] = [
  { value: 'game', label: 'Game' },
  { value: 'launcher', label: 'Launcher' },
  { value: 'util', label: 'Utility' },
];

/** Derive a friendly display name from an executable path's file name. */
function suggestName(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  const stem = base.replace(/\.(exe|bat|cmd|com|lnk|app)$/i, '');
  return stem
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function AllowListEditor() {
  const [entries, setEntries] = useState<LaunchEntry[]>(() => loadLaunchEntries());
  const [candidates, setCandidates] = useState<ScanCandidate[]>([]);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [manualName, setManualName] = useState('');
  const [manualPath, setManualPath] = useState('');
  const [manualCategory, setManualCategory] = useState<LaunchCategory>('game');
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
    setEntries(
      addLaunchEntry({
        name: manualName.trim(),
        executablePath: manualPath.trim(),
        category: manualCategory,
      }),
    );
    setManualName('');
    setManualPath('');
    setManualCategory('game');
  }

  async function browse() {
    const path = await pickExecutable();
    if (!path) return;
    setManualPath(path);
    if (!manualName.trim()) {
      const name = suggestName(path);
      setManualName(name);
      setManualCategory(categorizeByName(name));
    }
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
                <IconFallback name={entry.name} size={28} className="allow-list-icon" />
                <span className="allow-list-name">{entry.name}</span>
                <span className="allow-list-path">{entry.executablePath}</span>
                <span className="allow-list-actions">
                  <select
                    className="allow-list-category"
                    value={entryCategory(entry)}
                    aria-label={`Section for ${entry.name}`}
                    onChange={(e) =>
                      setEntries(setEntryCategory(entry.id, e.target.value as LaunchCategory))
                    }
                  >
                    {CATEGORY_OPTIONS.map((o) => (
                      <option key={o.value} value={o.value}>
                        {o.label}
                      </option>
                    ))}
                  </select>
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
                <IconFallback name={c.name} size={28} className="allow-list-icon" />
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
        <h2>Add an application</h2>
        <p className="hint">Browse for an executable, or paste its full path.</p>
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
          <select
            value={manualCategory}
            aria-label="Section"
            onChange={(e) => setManualCategory(e.target.value as LaunchCategory)}
          >
            {CATEGORY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <button type="button" className="secondary" onClick={() => void browse()}>
            Browse…
          </button>
          <button type="submit">Add</button>
        </form>
      </div>

      {status ? <p className="meta">{status}</p> : null}
    </div>
  );
}
