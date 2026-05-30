import { useKiosk } from '../context/KioskProvider';

export function SessionPage() {
  const { playerName, activeSession, wsConnected, lastEvent, playerLogout } = useKiosk();

  const minutes = activeSession?.remainingMinutes?.toFixed(1) ?? '—';

  return (
    <section className="panel session">
      <h1>Welcome, {playerName}</h1>
      <p className="session-timer">Time remaining: {minutes} min</p>
      <p className="meta">
        Realtime: {wsConnected ? 'connected' : 'reconnecting…'}
        {lastEvent ? ` · Last event: ${lastEvent}` : ''}
      </p>
      <p className="hint">Launched games respawn automatically if closed.</p>
      <button type="button" className="secondary" onClick={() => void playerLogout()}>
        End session
      </button>
    </section>
  );
}
