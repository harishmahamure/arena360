import { useEffect, useState } from 'react';
import { useKiosk } from '../context/KioskProvider';
import { clearFailures, getLockout, recordFailure } from '../lib/loginLockout';

function formatRetry(retryAt: number): string {
  const mins = Math.max(1, Math.ceil((retryAt - Date.now()) / 60000));
  return `${mins} minute${mins === 1 ? '' : 's'}`;
}

export function PlayerLoginPage() {
  const { playerLogin, enterSetup, error } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [lockout, setLockout] = useState(() => getLockout());

  // Re-evaluate the lockout window every 10s so it lifts without a reload.
  useEffect(() => {
    if (!lockout.locked) return;
    const id = setInterval(() => setLockout(getLockout()), 10000);
    return () => clearInterval(id);
  }, [lockout.locked]);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (lockout.locked) return;
    setBusy(true);
    try {
      await playerLogin(username, password);
      clearFailures();
    } catch {
      setLockout(recordFailure());
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel">
      <h1>Player sign in</h1>
      <form onSubmit={onSubmit}>
        <label>
          Username
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            disabled={lockout.locked}
            required
          />
        </label>
        <label>
          Password
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={lockout.locked}
            required
          />
        </label>
        {lockout.locked ? (
          <div className="error-panel" role="alert">
            <p className="error-headline">Too many attempts</p>
            <p className="error-detail">
              Sign-in is locked. Try again in {formatRetry(lockout.retryAt)} or ask staff for help.
            </p>
          </div>
        ) : error ? (
          <div className="error-panel" role="alert">
            {error.split('\n').map((line) => (
              <p
                key={line}
                className={line.startsWith('No usable plan') ? 'error-headline' : 'error-detail'}
              >
                {line}
              </p>
            ))}
          </div>
        ) : null}
        <button type="submit" disabled={busy || lockout.locked}>
          {busy ? 'Signing in…' : 'Start session'}
        </button>
      </form>
      <button type="button" className="link" onClick={() => void enterSetup()}>
        Administrator setup
      </button>
    </section>
  );
}
