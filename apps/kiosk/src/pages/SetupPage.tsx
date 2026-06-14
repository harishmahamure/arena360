import { useAsyncAction } from '@gaming-cafe/utils';
import { useEffect, useRef, useState } from 'react';
import { AllowListEditor } from '../components/AllowListEditor';
import { AsyncActionButton } from '../components/AsyncActionButton';
import { useKiosk } from '../context/KioskProvider';
import { KIOSK_LOGO_URL } from '../lib/config';

const SETUP_IDLE_MS = 15 * 60 * 1000;

export function SetupPage() {
  const { requestAdminOtp, adminLogin, exitSetup, factoryReset, error } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [otp, setOtp] = useState('');
  const [sessionOtpId, setSessionOtpId] = useState<string | null>(null);
  const [authenticated, setAuthenticated] = useState(false);
  const [busy, setBusy] = useState(false);
  const lockAction = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const factoryResetAction = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const setupActionLoading = lockAction.loading || factoryResetAction.loading;
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  async function requestOtp(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      const id = await requestAdminOtp(username, password);
      setSessionOtpId(id);
    } catch {
      // context error
    } finally {
      setBusy(false);
    }
  }

  async function verifyOtp(e: React.FormEvent) {
    e.preventDefault();
    if (!sessionOtpId) return;
    setBusy(true);
    try {
      await adminLogin(username, password, otp, sessionOtpId);
      setAuthenticated(true);
    } catch {
      // context error
    } finally {
      setBusy(false);
    }
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

        {!sessionOtpId ? (
          <form className="setup-login-form" onSubmit={requestOtp}>
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
                {busy ? 'Sending…' : 'Send OTP'}
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
          <form className="setup-login-form" onSubmit={verifyOtp}>
            <label>
              OTP code
              <input
                value={otp}
                onChange={(e) => setOtp(e.target.value)}
                required
                autoComplete="one-time-code"
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
                  setSessionOtpId(null);
                  setOtp('');
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
