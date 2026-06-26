import { useKiosk } from '../context/KioskProvider';
import { KIOSK_LOGO_URL } from '../lib/config';

export function CreateAccountSuccessPage() {
  const { registeredUsername, backToLoginFromCreateAccount } = useKiosk();

  return (
    <div className="a360-login login-home">
      <div className="a360-radial-overlay" />

      <main className="a360-login-card">
        <header className="a360-login-header">
          {KIOSK_LOGO_URL ? (
            <img className="a360-login-logo" src={KIOSK_LOGO_URL} alt="" />
          ) : (
            <span className="a360-brand">ARENA360</span>
          )}
          <h1 className="a360-login-title">Account created</h1>
          <p className="a360-login-subtitle">
            {registeredUsername
              ? `Welcome, ${registeredUsername}! Ask staff to add gaming time, then sign in.`
              : 'Ask staff to add gaming time, then sign in.'}
          </p>
        </header>

        <button type="button" className="primary-glow-btn" onClick={backToLoginFromCreateAccount}>
          Back to sign in
        </button>
      </main>
    </div>
  );
}
