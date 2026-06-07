import type { TrackedApp } from './useTrackedProcesses';

interface RunningAppsBarProps {
  processes: TrackedApp[];
  closing: boolean;
  onCloseAll: () => void;
}

/** Shown while allow-listed apps launched from the kiosk are still running. */
export function RunningAppsBar({ processes, closing, onCloseAll }: RunningAppsBarProps) {
  if (processes.length === 0) return null;

  const primary = processes[0];
  const label =
    processes.length === 1 && primary ? primary.displayName : `${processes.length} apps running`;

  return (
    <section className="a360-running-apps" aria-label="Running applications">
      <div className="a360-running-apps-inner glass-card">
        <div className="a360-running-apps-copy">
          <span className="material-symbols-outlined a360-running-apps-icon" aria-hidden="true">
            sports_esports
          </span>
          <div>
            <p className="a360-running-apps-title">Running: {label}</p>
            {processes.length === 1 && primary ? (
              <p className="a360-running-apps-sub" title={primary.executablePath}>
                Use <strong>Alt+Tab</strong> to switch between games and Arena360 only. Press{' '}
                <strong>Ctrl+Shift+H</strong> to return to Arena360, then close when you are done
                playing.
              </p>
            ) : (
              <>
                <p className="a360-running-apps-sub">
                  Use <strong>Alt+Tab</strong> to switch between games and Arena360 only. Press{' '}
                  <strong>Ctrl+Shift+H</strong> to return to Arena360.
                </p>
                <ul className="a360-running-apps-list">
                  {processes.map((process) => (
                    <li key={process.pid} title={process.executablePath}>
                      {process.displayName}
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </div>
        <button
          type="button"
          className="a360-running-apps-close danger"
          disabled={closing}
          onClick={() => void onCloseAll()}
        >
          {closing ? 'Closing…' : 'Close app'}
        </button>
      </div>
    </section>
  );
}
