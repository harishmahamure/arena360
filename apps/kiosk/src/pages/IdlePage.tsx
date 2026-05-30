import { useKiosk } from '../context/KioskProvider';

export function IdlePage() {
  const { deviceName, wsConnected, goToPlayerLogin, enterSetup, factoryReset } = useKiosk();

  return (
    <section className="panel">
      <h1>{deviceName ?? 'Kiosk ready'}</h1>
      <p className="meta">WebSocket: {wsConnected ? 'connected' : 'offline'}</p>
      <button type="button" onClick={goToPlayerLogin}>
        Player sign in
      </button>
      <button type="button" className="secondary" onClick={() => void enterSetup()}>
        Administrator setup
      </button>
      <button type="button" className="link danger" onClick={() => void factoryReset()}>
        Factory reset
      </button>
    </section>
  );
}
