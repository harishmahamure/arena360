import { SetupGesture } from '../components/SetupGesture';
import { useKiosk } from '../context/KioskProvider';

export function IdlePage() {
  const {
    deviceName,
    wsConnected,
    online,
    maintenance,
    deviceStatus,
    goToPlayerLogin,
    enterSetup,
  } = useKiosk();

  return (
    <section className="panel attract">
      <SetupGesture onTrigger={() => void enterSetup()} />
      <h1>{deviceName ?? 'Game Zone'}</h1>
      <p className="meta">{wsConnected ? 'Ready' : 'Reconnecting…'}</p>

      {maintenance ? (
        <div className="maintenance-banner" role="alert">
          <p className="error-headline">
            {deviceStatus === 'out_of_service'
              ? 'This station is out of service.'
              : 'This station is under maintenance.'}
          </p>
          <p className="error-detail">Please ask staff for another station.</p>
        </div>
      ) : !online ? (
        <div className="maintenance-banner" role="alert">
          <p className="error-headline">This station is offline.</p>
          <p className="error-detail">Sign-in is disabled until the connection returns.</p>
        </div>
      ) : null}

      <button
        type="button"
        className="attract-cta"
        onClick={goToPlayerLogin}
        disabled={maintenance || !online}
      >
        Tap to start
      </button>
    </section>
  );
}
