import { useKiosk } from '../context/KioskProvider';
import { KIOSK_APP_VERSION } from '../lib/config';

/** Client-side station health snapshot for setup / fleet diagnostics (no new API). */
export function StationHealthPanel() {
  const { deviceName, deviceStatus, wsConnected, online, phase } = useKiosk();

  return (
    <section className="allow-list-section station-health" aria-label="Station health">
      <h3 className="allow-list-subhead">Station health</h3>
      <dl className="station-health-grid">
        <div>
          <dt>App version</dt>
          <dd>{KIOSK_APP_VERSION}</dd>
        </div>
        <div>
          <dt>Station</dt>
          <dd>{deviceName ?? '—'}</dd>
        </div>
        <div>
          <dt>Phase</dt>
          <dd>{phase}</dd>
        </div>
        <div>
          <dt>API</dt>
          <dd>{online ? 'Reachable' : 'Unreachable'}</dd>
        </div>
        <div>
          <dt>WebSocket</dt>
          <dd>{wsConnected ? 'Connected' : 'Disconnected'}</dd>
        </div>
        <div>
          <dt>Device status</dt>
          <dd>{deviceStatus ?? 'unknown'}</dd>
        </div>
      </dl>
    </section>
  );
}
