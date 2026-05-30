import { AuthFormField, AuthShell, AuthSubmitButton } from '@gaming-cafe/ui/primitives';
import { useEffect, useState } from 'react';
import { BackgroundMedia } from '../components/BackgroundMedia';
import { StationControls } from '../components/StationControls';
import { useKiosk } from '../context/KioskProvider';
import { KIOSK_LOGO_URL } from '../lib/config';
import { fetchActiveGames, type Game, pickBackgroundGame } from '../lib/games';
import { clearFailures, getLockout, recordFailure } from '../lib/loginLockout';

function formatRetry(retryAt: number): string {
  const mins = Math.max(1, Math.ceil((retryAt - Date.now()) / 60000));
  return `${mins} minute${mins === 1 ? '' : 's'}`;
}

/**
 * Default logged-out home (ggLeap-style): full-screen branded background, center
 * logo, and the player sign-in form. Maintenance/offline states surface here.
 * Setup is reached via Ctrl+Shift+A (handled globally in KioskProvider).
 */
export function LoginHomePage() {
  const { playerLogin, error, online, maintenance, deviceName } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [lockout, setLockout] = useState(() => getLockout());
  const [backgroundGame, setBackgroundGame] = useState<Game | undefined>(undefined);

  useEffect(() => {
    let active = true;
    fetchActiveGames()
      .then((games) => {
        if (active) setBackgroundGame(pickBackgroundGame(games));
      })
      .catch(() => {
        // No catalog yet — degrade to the plain dark surface.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!lockout.locked) return;
    const id = setInterval(() => setLockout(getLockout()), 10000);
    return () => clearInterval(id);
  }, [lockout.locked]);

  const blocked = maintenance || !online || lockout.locked;

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (blocked) return;
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
    <AuthShell
      className="login-home"
      backgroundSlot={<BackgroundMedia game={backgroundGame} showScrim={false} />}
    >
      <header className="gz-auth-form-header">
        {KIOSK_LOGO_URL ? <img className="gz-auth-logo" src={KIOSK_LOGO_URL} alt="" /> : null}
        <h1 className="gz-auth-heading">Welcome back</h1>
        <p className="gz-auth-subtitle">Enter your credentials to start your session</p>
      </header>

      {maintenance ? (
        <div className="maintenance-banner" role="alert">
          <p className="error-headline">Station under maintenance</p>
          <p className="error-detail">Please ask staff for assistance.</p>
        </div>
      ) : !online ? (
        <div className="maintenance-banner" role="alert">
          <p className="error-headline">Reconnecting…</p>
          <p className="error-detail">Sign-in is unavailable until the connection returns.</p>
        </div>
      ) : null}

      <form className="gz-auth-form" onSubmit={onSubmit}>
        <AuthFormField
          label="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          disabled={blocked}
          autoComplete="off"
          required
        />
        <AuthFormField
          label="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={blocked}
          required
        />

        {lockout.locked ? (
          <div className="gz-auth-error" role="alert">
            <p className="error-headline">Too many attempts</p>
            <p className="error-detail">
              Sign-in is locked. Try again in {formatRetry(lockout.retryAt)} or ask staff for help.
            </p>
          </div>
        ) : error ? (
          <div className="gz-auth-error" role="alert">
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

        <AuthSubmitButton type="submit" busy={busy} disabled={blocked}>
          {busy ? 'Signing in…' : 'Sign in'}
        </AuthSubmitButton>
      </form>

      <StationControls deviceName={deviceName} online={online} maintenance={maintenance} />
    </AuthShell>
  );
}
