import { useState } from 'react';
import { lockWorkstation, restartStation, shutdownStation } from '../lib/tauriCommands';

type PowerAction = 'restart' | 'shutdown';

interface StationControlsProps {
  deviceName: string | null;
  online: boolean;
  maintenance: boolean;
}

function statusLabel({
  online,
  maintenance,
}: Pick<StationControlsProps, 'online' | 'maintenance'>) {
  if (maintenance) return 'Maintenance';
  if (!online) return 'Offline';
  return 'Online';
}

export function StationControls({ deviceName, online, maintenance }: StationControlsProps) {
  const [confirm, setConfirm] = useState<PowerAction | null>(null);
  const [busy, setBusy] = useState<PowerAction | 'lock' | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function runLock() {
    setBusy('lock');
    setError(null);
    try {
      await lockWorkstation();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not lock this PC');
    } finally {
      setBusy(null);
    }
  }

  async function runPower(action: PowerAction) {
    setBusy(action);
    setError(null);
    try {
      if (action === 'restart') await restartStation();
      else await shutdownStation();
    } catch (e) {
      setError(e instanceof Error ? e.message : `Could not ${action} this PC`);
      setBusy(null);
      setConfirm(null);
    }
  }

  const label = statusLabel({ online, maintenance });

  return (
    <section className="station-controls" aria-label="Station controls">
      <div className="station-controls-header">
        <span className="station-controls-title">{deviceName ?? 'Station'}</span>
        <span className={`station-status station-status-${label.toLowerCase()}`}>{label}</span>
      </div>

      <div className="station-controls-actions">
        <button
          type="button"
          className="secondary station-control-button"
          disabled={busy !== null}
          onClick={() => void runLock()}
        >
          {busy === 'lock' ? 'Locking…' : 'Lock'}
        </button>
        <button
          type="button"
          className="secondary station-control-button"
          disabled={busy !== null}
          onClick={() => setConfirm('restart')}
        >
          Restart
        </button>
        <button
          type="button"
          className="secondary danger station-control-button"
          disabled={busy !== null}
          onClick={() => setConfirm('shutdown')}
        >
          Shutdown
        </button>
      </div>

      <p className="station-controls-hint">Admin setup: Ctrl+Shift+A</p>

      {error ? (
        <p className="station-controls-error" role="alert">
          {error}
        </p>
      ) : null}

      {confirm ? (
        <div className="station-power-confirm" role="alertdialog" aria-modal="false">
          <p>
            {confirm === 'restart'
              ? 'Restart this PC now? Active work may be lost.'
              : 'Shut down this PC now? Active work may be lost.'}
          </p>
          <div className="station-power-actions">
            <button
              type="button"
              className="danger"
              disabled={busy !== null}
              onClick={() => void runPower(confirm)}
            >
              {busy === confirm
                ? confirm === 'restart'
                  ? 'Restarting…'
                  : 'Shutting down…'
                : confirm === 'restart'
                  ? 'Yes, restart'
                  : 'Yes, shut down'}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy !== null}
              onClick={() => setConfirm(null)}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : null}
    </section>
  );
}
