import { useState } from 'react';
import { useKiosk } from '../context/KioskProvider';

export function SetupPage() {
  const { requestAdminOtp, adminLogin, exitSetup, error } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [otp, setOtp] = useState('');
  const [sessionOtpId, setSessionOtpId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
    } catch {
      // context error
    } finally {
      setBusy(false);
    }
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
            Verify and exit setup
          </button>
        </form>
      )}
      <button type="button" className="secondary" onClick={() => void exitSetup()}>
        Cancel
      </button>
    </section>
  );
}
