import { useMemo, useState } from 'react';
import { allowListPaths, type LaunchEntry, loadLaunchEntries } from '../lib/allowList';
import { launchAllowed } from '../lib/tauriCommands';

interface LauncherGridProps {
  /** Disable launching (e.g. during force-end grace). */
  disabled?: boolean;
  onError?: (message: string) => void;
}

/**
 * Player-facing launcher. Renders the curated allow-list (ADR-0019) and calls
 * the native `launch_allowed` guard, which re-validates the path server-side
 * of the trust boundary (the Rust command) against the allow-list snapshot.
 */
export function LauncherGrid({ disabled, onError }: LauncherGridProps) {
  const entries = useMemo<LaunchEntry[]>(
    // Hide entries whose executable was missing at last scan.
    () => loadLaunchEntries().filter((e) => e.present !== false),
    [],
  );
  const [launchingId, setLaunchingId] = useState<string | null>(null);

  async function launch(entry: LaunchEntry) {
    if (disabled) return;
    setLaunchingId(entry.id);
    try {
      await launchAllowed(entry.executablePath, allowListPaths(entries), entry.arguments);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : `Could not launch ${entry.name}`);
    } finally {
      setLaunchingId(null);
    }
  }

  if (entries.length === 0) {
    return <p className="hint">No games configured. Ask staff to set up this station.</p>;
  }

  return (
    <div className="launcher-grid">
      {entries.map((entry) => (
        <button
          key={entry.id}
          type="button"
          className="launcher-tile"
          disabled={disabled || launchingId !== null}
          onClick={() => void launch(entry)}
        >
          <span className="launcher-tile-name">{entry.name}</span>
          {launchingId === entry.id ? (
            <span className="launcher-tile-status">Launching…</span>
          ) : null}
        </button>
      ))}
    </div>
  );
}
