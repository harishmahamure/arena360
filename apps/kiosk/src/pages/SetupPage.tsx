import { useEffect, useRef, useState } from 'react';
import { AllowListEditor } from '../components/AllowListEditor';
import { useKiosk } from '../context/KioskProvider';

const SETUP_IDLE_MS = 15 * 60 * 1000;

export function SetupPage() {
  const { requestAdminOtp, adminLogin, exitSetup, factoryReset, error } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [otp, setOtp] = useState('');
  const [sessionOtpId, setSessionOtpId] = useState<string | null>(null);
  const [authenticated, setAuthenticated] = useState(false);
  const [busy, setBusy] = useState(false);
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Re-lock setup mode after 15 minutes of inactivity (ADR-0020).
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
      // adminLogin relaxes lockdown but stays in setup; reveal the editor.
      setAuthenticated(true);
    } catch {
      // context error
    } finally {
      setBusy(false);
    }
  }

  // Setup mode: while authenticated and still on the page, show the allow-list editor.
  if (authenticated) {
    return (
      <section className="panel panel-wide">
        <h1>Setup — allow-list</h1>
        <AllowListEditor />
        <div className="setup-actions">
          <button type="button" className="secondary" onClick={() => void exitSetup()}>
            Done — re-lock
          </button>
          <button type="button" className="link danger" onClick={() => void factoryReset()}>
            Factory reset
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="panel">
      <h1>Administrator setup</h1>
      <p>Admin login only. Lockdown is relaxed while you are here.</p>
      {!sessionOtpId ? (
        <form onSubmit={requestOtp}>
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
          <button type="submit" disabled={busy}>
            Send OTP
          </button>
        </form>
      ) : (
        <form onSubmit={verifyOtp}>
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
          <button type="submit" disabled={busy}>
            Verify and open setup
          </button>
        </form>
      )}
      <button type="button" className="secondary" onClick={() => void exitSetup()}>
        Cancel
      </button>
    </section>
  );
}
