import { useKiosk } from '../context/KioskProvider';

/**
 * Shown when the single-login guard rejects a start with
 * `PLAYER_ALREADY_IN_SESSION` (ADR-0017). The player already has an open
 * session on another station and must end it there first.
 */
export function AlreadyInSessionPage() {
  const { conflictDevice, dismissConflict } = useKiosk();

  return (
    <section className="panel">
      <h1>Already signed in</h1>
      <div className="error-panel" role="alert">
        <p className="error-headline">You're already in a session.</p>
        <p className="error-detail">
          Your account has an active session on{' '}
          <strong>{conflictDevice ?? 'another station'}</strong>. End that session before starting a
          new one.
        </p>
      </div>
      <button type="button" onClick={dismissConflict}>
        Back
      </button>
    </section>
  );
}
