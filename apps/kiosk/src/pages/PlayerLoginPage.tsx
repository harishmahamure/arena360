import { useState } from 'react';
import { useKiosk } from '../context/KioskProvider';

export function PlayerLoginPage() {
  const { playerLogin, enterSetup, error } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await playerLogin(username, password);
    } catch {
      // handled in context
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
          <input value={username} onChange={(e) => setUsername(e.target.value)} required />
        </label>
        <label>
          Password
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </label>
        {error ? (
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
        <button type="submit" disabled={busy}>
          {busy ? 'Signing in…' : 'Start session'}
        </button>
      </form>
      <button type="button" className="link" onClick={() => void enterSetup()}>
        Administrator setup
      </button>
    </section>
  );
}
