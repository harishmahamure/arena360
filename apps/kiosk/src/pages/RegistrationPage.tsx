import { useState } from 'react';
import { useKiosk } from '../context/KioskProvider';

export function RegistrationPage() {
  const { registerDevice, error } = useKiosk();
  const [code, setCode] = useState('');
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await registerDevice(code.trim().toUpperCase(), name.trim());
    } catch {
      // error set in context
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
        {error ? <p className="error">{error}</p> : null}
        <button type="submit" disabled={busy}>
          {busy ? 'Registering…' : 'Register device'}
        </button>
      </form>
    </section>
  );
}
