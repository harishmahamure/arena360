import { type ReactNode, useEffect } from 'react';
import { BootErrorOverlay } from '../components/BootErrorOverlay';
import { BootLoading } from '../components/BootLoading';
import { LockdownOverlay } from '../components/LockdownOverlay';
import { KioskProvider, useKiosk } from '../context/KioskProvider';
import { useKioskShellGuard } from '../lib/useKioskShellGuard';
import { AlreadyInSessionPage } from '../pages/AlreadyInSessionPage';
import { LoginHomePage } from '../pages/LoginHomePage';
import { RegistrationPage } from '../pages/RegistrationPage';
import { SessionPage } from '../pages/SessionPage';
import { SetupPage } from '../pages/SetupPage';

function KioskShell() {
  const { phase, error, refresh } = useKiosk();
  const locked = phase !== 'setup' && phase !== 'loading' && phase !== 'boot-error';
  useKioskShellGuard(locked);

  // Auto-update manager (ADR-0028): check while idle (register, setup, login),
  // never during an active session. Loaded lazily so the Tauri updater plugin
  // is not pulled into browser/test bundles.
  useEffect(() => {
    void import('../lib/updater').then((m) => {
      m.setUpdatePhase(phase);
      if (m.isIdleUpdatePhase(phase)) {
        void m.checkForUpdateWhenIdle(phase);
      }
    });
  }, [phase]);

  let content: ReactNode = <BootLoading />;
  if (phase === 'boot-error') {
    content = null;
  } else if (phase === 'register') content = <RegistrationPage />;
  else if (phase === 'login') content = <LoginHomePage />;
  else if (phase === 'setup') content = <SetupPage />;
  else if (phase === 'session') content = <SessionPage />;
  else if (phase === 'already-in-session') content = <AlreadyInSessionPage />;

  return (
    <>
      <LockdownOverlay visible={locked}>{content}</LockdownOverlay>
      {phase === 'boot-error' ? (
        <BootErrorOverlay message={error} onRetry={() => void refresh()} />
      ) : null}
    </>
  );
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
