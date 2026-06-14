import { ApiError, useAsyncAction } from '@gaming-cafe/utils';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useRef, useState } from 'react';
import { AllowListEditor } from '../components/AllowListEditor';
import { AsyncActionButton } from '../components/AsyncActionButton';
import { useKiosk } from '../context/KioskProvider';
import { KIOSK_LOGO_URL } from '../lib/config';
import { totpInputProps } from '../lib/inputHints';
import { setWatchdogPause } from '../lib/tauriCommands';

const SETUP_IDLE_MS = 15 * 60 * 1000;

type SetupLoginStep = 'credentials' | 'totp';

export function SetupPage() {
  const {
    adminLogin,
    exitSetup,
    factoryReset,
    error,
    setupAuthenticated,
    clearSetupAuthenticated,
  } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [totp, setTotp] = useState('');
  const [loginStep, setLoginStep] = useState<SetupLoginStep>('credentials');
  const [authenticated, setAuthenticated] = useState(false);
  const [busy, setBusy] = useState(false);
  const lockAction = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const factoryResetAction = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const exitDesktopAction = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const setupActionLoading =
    lockAction.loading || factoryResetAction.loading || exitDesktopAction.loading;
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!setupAuthenticated) return;
    setAuthenticated(true);
    clearSetupAuthenticated();
  }, [setupAuthenticated, clearSetupAuthenticated]);

  useEffect(() => {
    if (!authenticated) return;
    const reset = () => {
      if (idleTimerRef.current) clearTimeout(idleTimerRef.current);
      idleTimerRef.current = setTimeout(() => void exitSetup(), SETUP_IDLE_MS);
    };
    const events = ['pointerdown', 'keydown', 'pointermove'] as const;
    for (const evt of events) window.addEventListener(evt, reset);
    reset();
    return () => {
      for (const evt of events) window.removeEventListener(evt, reset);
      if (idleTimerRef.current) clearTimeout(idleTimerRef.current);
    };
  }, [authenticated, exitSetup]);

  async function submitCredentials(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await adminLogin(username, password, undefined, { relaxLockdown: true });
      setAuthenticated(true);
    } catch (err) {
      if (err instanceof ApiError && err.message === 'TOTP code is required') {
        setLoginStep('totp');
      }
    } finally {
      setBusy(false);
    }
  }

  async function submitTotp(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await adminLogin(username, password, totp.trim(), { relaxLockdown: true });
      setAuthenticated(true);
    } catch {
      // context error
    } finally {
      setBusy(false);
    }
  }

  async function exitToDesktop() {
    await setWatchdogPause(15, 'maintenance');
    await getCurrentWindow().close();
  }

  if (authenticated) {
    return (
      <div className="setup-shell">
        <section className="panel panel-setup">
          <h1>Setup</h1>
          <p className="hint">
            Scan and allow software on this station. Media is picked from the central CDN gallery.
          </p>
          <AllowListEditor />
          <div className="setup-actions">
            <AsyncActionButton
              className="secondary"
              loading={lockAction.loading}
              success={lockAction.succeeded}
              successLabel="Station locked"
              error={lockAction.failed}
              errorLabel={lockAction.errorMessage ?? 'Could not lock station'}
              loadingLabel="Locking…"
              disabled={lockAction.disabled || factoryResetAction.loading}
              onClick={() => void lockAction.run(() => exitSetup())}
            >
              Done — re-lock
            </AsyncActionButton>
            <AsyncActionButton
              className="secondary"
              loading={exitDesktopAction.loading}
              success={exitDesktopAction.succeeded}
              successLabel="Desktop available"
              error={exitDesktopAction.failed}
              errorLabel={exitDesktopAction.errorMessage ?? 'Could not exit to desktop'}
              loadingLabel="Preparing…"
              disabled={
                exitDesktopAction.disabled || lockAction.loading || factoryResetAction.loading
              }
              onClick={() => void exitDesktopAction.run(() => exitToDesktop())}
            >
              Exit to desktop (15 min)
            </AsyncActionButton>
            <AsyncActionButton
              className="link danger"
              loading={factoryResetAction.loading}
              success={factoryResetAction.succeeded}
              successLabel="Reset complete"
              error={factoryResetAction.failed}
              errorLabel={factoryResetAction.errorMessage ?? 'Factory reset failed'}
              loadingLabel="Resetting…"
              disabled={factoryResetAction.disabled || lockAction.loading}
              onClick={() => void factoryResetAction.run(() => factoryReset())}
            >
              Factory reset
            </AsyncActionButton>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="setup-login-shell">
      <section className="setup-login-card">
        <header className="setup-login-header">
          {KIOSK_LOGO_URL ? (
            <img className="a360-login-logo" src={KIOSK_LOGO_URL} alt="" />
          ) : (
            <span className="a360-brand">ARENA360</span>
          )}
          <h1 className="setup-login-title">Administrator setup</h1>
          <p className="setup-login-subtitle">
            Admin login only. Lockdown is relaxed while you are here.
          </p>
        </header>

        {loginStep === 'credentials' ? (
          <form className="setup-login-form" onSubmit={submitCredentials}>
            <label>
              Admin username
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
            {error ? <p className="error">{error}</p> : null}
            <div className="setup-login-actions">
              <button type="submit" disabled={busy}>
                {busy ? 'Signing in…' : 'Sign in'}
              </button>
              <button
                type="button"
                className="secondary"
                disabled={busy || setupActionLoading}
                onClick={() => void lockAction.run(() => exitSetup())}
              >
                Cancel
              </button>
            </div>
          </form>
        ) : (
          <form className="setup-login-form" onSubmit={submitTotp}>
            <label>
              Authenticator code
              <input
                value={totp}
                onChange={(e) => setTotp(e.target.value.replace(/\s+/g, '').slice(0, 6))}
                {...totpInputProps}
                required
              />
            </label>
            {error ? <p className="error">{error}</p> : null}
            <div className="setup-login-actions">
              <button type="submit" disabled={busy}>
                {busy ? 'Verifying…' : 'Verify and open setup'}
              </button>
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  setLoginStep('credentials');
                  setTotp('');
                }}
              >
                Back
              </button>
            </div>
          </form>
        )}
      </section>
    </div>
  );
}
