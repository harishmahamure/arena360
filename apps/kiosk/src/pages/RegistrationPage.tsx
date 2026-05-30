import { deviceSubTypeOptions, deviceTypeOptions } from '@gaming-cafe/contracts';
import { useEffect, useState } from 'react';
import { useKiosk } from '../context/KioskProvider';
import { collectFingerprint, type FingerprintPayload } from '../lib/tauriCommands';

export function RegistrationPage() {
  const { registerDevice, error } = useKiosk();
  const [code, setCode] = useState('');
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

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await registerDevice({
        code: code.trim().toUpperCase(),
        name: name.trim(),
        deviceType,
        deviceSubType,
        location: location.trim(),
        serialNumber: fingerprint?.serial,
      });
    } catch {
      // error surfaced via context
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel">
      <h1>Register this kiosk</h1>
      <p>Enter the one-time code from the admin portal.</p>
      <form onSubmit={onSubmit}>
        <label>
          Registration code
          <input
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="ABC-123"
            required
            autoComplete="off"
          />
        </label>
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
              <dt>MAC</dt>
              <dd>{fingerprint.mac}</dd>
              <dt>Serial</dt>
              <dd>{fingerprint.serial}</dd>
              <dt>BIOS UUID</dt>
              <dd>{fingerprint.biosUuid}</dd>
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
