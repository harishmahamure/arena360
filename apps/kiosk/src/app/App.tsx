import { LockdownOverlay } from '../components/LockdownOverlay';
import { KioskProvider, useKiosk } from '../context/KioskProvider';
import { IdlePage } from '../pages/IdlePage';
import { PlayerLoginPage } from '../pages/PlayerLoginPage';
import { RegistrationPage } from '../pages/RegistrationPage';
import { SessionPage } from '../pages/SessionPage';
import { SetupPage } from '../pages/SetupPage';
import './app.css';

function KioskShell() {
  const { phase } = useKiosk();
  const locked = phase !== 'setup' && phase !== 'loading';

  let content = <p className="meta">Loading…</p>;
  if (phase === 'register') content = <RegistrationPage />;
  else if (phase === 'idle') content = <IdlePage />;
  else if (phase === 'setup') content = <SetupPage />;
  else if (phase === 'player-login') content = <PlayerLoginPage />;
  else if (phase === 'session') content = <SessionPage />;

  return <LockdownOverlay visible={locked}>{content}</LockdownOverlay>;
}

export function App() {
  return (
    <KioskProvider>
      <main className="app">
        <KioskShell />
      </main>
    </KioskProvider>
  );
}
