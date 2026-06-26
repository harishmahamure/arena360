import { memo } from 'react';
import { fetchTools } from '../../lib/allowList';
import { openAudioSettings } from '../../lib/tauriCommands';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

interface SettingsViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
  onLaunched?: () => void;
}

/**
 * Arena360 Settings & Tools: launch allowed launcher/utility apps (ADR-0019),
 * plus a built-in Sound Settings tile.
 */
export const SettingsView = memo(function SettingsView({
  disabled,
  onError,
  onLaunched,
}: SettingsViewProps) {
  const { items: tools } = useAllowList(fetchTools);
  const { launchingKey, isLaunchable, launch } = useLauncher(
    Boolean(disabled),
    onError,
    onLaunched,
  );

  return (
    <section className="a360-section">
      <header className="a360-settings-head">
        <h1 className="a360-settings-title">System Settings &amp; Tools</h1>
        <p className="a360-settings-sub">
          Launch the utility applications allowed on this station. After opening Sound Settings, use{' '}
          <strong>Alt+Tab</strong> to return to Arena360. Use <strong>Close app</strong> in the
          running-apps bar when you are done with other tools.
        </p>
      </header>

      <div className="a360-tools-grid">
        {tools.map((tool) => {
          const launchable = isLaunchable(tool);
          return (
            <div key={tool.id} className="glass-card a360-tool-card">
              <div>
                <span className="a360-tool-icon">
                  <span className="material-symbols-outlined">{tool.icon ?? 'build'}</span>
                </span>
                <h3 className="a360-tool-name">{tool.name}</h3>
                <p className="a360-tool-sub">{tool.subtitle ?? tool.description ?? 'Utility'}</p>
              </div>
              <button
                type="button"
                className="a360-tool-launch"
                disabled={!launchable || disabled || launchingKey !== null}
                onClick={() => void launch(tool)}
              >
                <span>{launchingKey === tool.id ? 'Launching…' : 'Launch'}</span>
                <span className="material-symbols-outlined">open_in_new</span>
              </button>
            </div>
          );
        })}

        <div className="glass-card a360-tool-card">
          <div>
            <span className="a360-tool-icon">
              <span className="material-symbols-outlined">volume_up</span>
            </span>
            <h3 className="a360-tool-name">Sound Settings</h3>
            <p className="a360-tool-sub">Audio Config</p>
          </div>
          <button
            type="button"
            className="a360-tool-launch"
            disabled={disabled}
            onClick={() =>
              void openAudioSettings().catch((error) =>
                onError?.(error instanceof Error ? error.message : 'Could not open sound settings'),
              )
            }
          >
            <span>Open</span>
            <span className="material-symbols-outlined">open_in_new</span>
          </button>
        </div>
      </div>
    </section>
  );
});
