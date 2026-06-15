import { deviceSubTypeOptions, deviceTypeOptions } from '@gaming-cafe/contracts';
import { ApiError, normalizeUsername, sanitizeUsernameInput, trimValue } from '@gaming-cafe/utils';
import { useEffect, useState } from 'react';
import { useKiosk } from '../context/KioskProvider';
import { totpInputProps } from '../lib/inputHints';
import { collectFingerprint, type FingerprintPayload } from '../lib/tauriCommands';

type Step = 'credentials' | 'totp' | 'device';

/**
 * First-time provisioning (DRAFT-0023): an administrator signs in on the device
 * (username/password, plus TOTP when enabled); the admin session then names and
 * registers this PC. No registration code is involved.
 */
export function RegistrationPage() {
  const { adminLogin, provisionDevice, adminAuthenticated, error } = useKiosk();

  const [step, setStep] = useState<Step>('credentials');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [totp, setTotp] = useState('');

  const [name, setName] = useState('');
  const [deviceType, setDeviceType] = useState<string>(deviceTypeOptions[0]?.value ?? 'PC');
  const [deviceSubType, setDeviceSubType] = useState<string>(
    deviceSubTypeOptions[0]?.value ?? 'HIGH_END_PCS',
  );
  const [location, setLocation] = useState('');
  const [fingerprint, setFingerprint] = useState<FingerprintPayload | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    collectFingerprint()
      .then(setFingerprint)
      .catch(() => setFingerprint(null));
  }, []);

  async function onCredentials(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await adminLogin(normalizeUsername(username), trimValue(password));
      setStep('device');
    } catch (err) {
      if (err instanceof ApiError && err.message === 'TOTP code is required') {
        setStep('totp');
      }
    } finally {
      setBusy(false);
    }
  }

  async function onTotp(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await adminLogin(normalizeUsername(username), trimValue(password), trimValue(totp));
      setStep('device');
    } catch {
      // surfaced via context error
    } finally {
      setBusy(false);
    }
  }

  async function onProvision(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await provisionDevice({
        name: name.trim(),
        deviceType,
        deviceSubType,
        location: location.trim(),
        serialNumber: fingerprint?.mac,
      });
    } catch {
      // surfaced via context error
    } finally {
      setBusy(false);
    }
  }

  if (step === 'credentials') {
    return (
      <section className="panel">
        <h1>Set up this station</h1>
        <p>Sign in with an administrator account to register this PC.</p>
        <form onSubmit={onCredentials}>
          <label>
            Admin username
            <input
              value={username}
              onChange={(e) => setUsername(sanitizeUsernameInput(e.target.value))}
              autoComplete="off"
              required
            />
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
            {busy ? 'Signing in…' : 'Continue'}
          </button>
        </form>
      </section>
    );
  }

  if (step === 'totp') {
    return (
      <section className="panel">
        <h1>Enter authenticator code</h1>
        <p>Enter the TOTP code from your authenticator app for {username}.</p>
        <form onSubmit={onTotp}>
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
          <button type="submit" disabled={busy}>
            {busy ? 'Verifying…' : 'Verify'}
          </button>
          <button type="button" className="link" onClick={() => setStep('credentials')}>
            Back
          </button>
        </form>
      </section>
    );
  }

  return (
    <section className="panel">
      <h1>Name this station</h1>
      <p>{adminAuthenticated ? 'Administrator verified.' : ''} Describe this PC to finish setup.</p>
      <form onSubmit={onProvision}>
        <label>
          Station name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="PC-01"
            required
          />
        </label>
        <label>
          Device type
          <select value={deviceType} onChange={(e) => setDeviceType(e.target.value)}>
            {deviceTypeOptions.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          Device sub-type
          <select value={deviceSubType} onChange={(e) => setDeviceSubType(e.target.value)}>
            {deviceSubTypeOptions.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          Location (optional)
          <input
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            placeholder="Floor 1 — Bay A"
          />
        </label>

        <fieldset className="fingerprint-preview">
          <legend>Hardware fingerprint</legend>
          {fingerprint ? (
            <dl>
              <dt>MAC (station identifier)</dt>
              <dd>{fingerprint.mac}</dd>
            </dl>
          ) : (
            <p className="hint">Collecting hardware fingerprint…</p>
          )}
        </fieldset>

        {error ? <p className="error">{error}</p> : null}
        <button type="submit" disabled={busy || !fingerprint}>
          {busy ? 'Registering…' : 'Register device'}
        </button>
      </form>
    </section>
  );
}
