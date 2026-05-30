import { useCallback, useEffect, useRef, useState } from 'react';
import { HudTimer } from '../components/HudTimer';
import { LauncherGrid } from '../components/LauncherGrid';
import { ToastHost, type ToastMessage } from '../components/Toast';
import { useKiosk } from '../context/KioskProvider';
import { useSessionPoller } from '../hooks/useSessionPoller';
import { OFFLINE_GRACE_MS } from '../lib/config';

const REMINDER_THRESHOLDS = [10, 5, 1] as const;

function formatGrace(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function SessionPage() {
  const {
    playerName,
    activeSession,
    wsConnected,
    online,
    forceEndGraceEndsAt,
    startSession,
    endSession,
    syncSession,
  } = useKiosk();

  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [graceLeft, setGraceLeft] = useState(0);
  const [offlineLeft, setOfflineLeft] = useState<number | null>(null);
  const offlineSinceRef = useRef<number | null>(null);
  const startedRef = useRef(false);
  const firedRemindersRef = useRef<Set<number>>(new Set());
  const prevMinutesRef = useRef<number | null>(null);

  const pushToast = useCallback((text: string, tone: ToastMessage['tone'] = 'info') => {
    setToasts((prev) => [...prev, { id: `${Date.now()}-${Math.random()}`, text, tone }]);
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // Create (or resume) the session once when arriving without one.
  useEffect(() => {
    if (!activeSession && !startedRef.current) {
      startedRef.current = true;
      void startSession();
    }
  }, [activeSession, startSession]);

  const remaining = activeSession?.remainingMinutes;
  useSessionPoller(remaining, syncSession, Boolean(activeSession) && forceEndGraceEndsAt === null);

  // Reminders + recharge toast when the authoritative value changes.
  useEffect(() => {
    if (typeof remaining !== 'number') return;
    const prev = prevMinutesRef.current;
    if (prev !== null && remaining > prev + 0.05) {
      pushToast('Time added — thanks!', 'success');
      // Re-arm reminders the player has climbed back above.
      for (const t of REMINDER_THRESHOLDS) {
        if (remaining > t) firedRemindersRef.current.delete(t);
      }
    }
    for (const t of REMINDER_THRESHOLDS) {
      if (remaining <= t && !firedRemindersRef.current.has(t)) {
        firedRemindersRef.current.add(t);
        pushToast(`${t} minute${t === 1 ? '' : 's'} remaining`, t === 1 ? 'warning' : 'info');
      }
    }
    prevMinutesRef.current = remaining;
  }, [remaining, pushToast]);

  // Auto-end when the countdown reaches zero (unless extended in time).
  useEffect(() => {
    if (typeof remaining === 'number' && remaining <= 0 && forceEndGraceEndsAt === null) {
      void endSession('auto');
    }
  }, [remaining, forceEndGraceEndsAt, endSession]);

  // Force-end grace overlay countdown -> cleanup at zero.
  useEffect(() => {
    if (forceEndGraceEndsAt === null) return;
    const tick = () => {
      const left = forceEndGraceEndsAt - Date.now();
      setGraceLeft(left);
      if (left <= 0) void endSession('force');
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [forceEndGraceEndsAt, endSession]);

  // Offline grace: keep counting down locally, then re-lock when grace elapses.
  useEffect(() => {
    if (online) {
      offlineSinceRef.current = null;
      setOfflineLeft(null);
      return;
    }
    if (offlineSinceRef.current === null) offlineSinceRef.current = Date.now();
    const tick = () => {
      const since = offlineSinceRef.current ?? Date.now();
      const left = OFFLINE_GRACE_MS - (Date.now() - since);
      setOfflineLeft(left);
      if (left <= 0) void endSession('offline_reconcile');
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [online, endSession]);

  if (forceEndGraceEndsAt !== null) {
    return (
      <section className="panel force-end-overlay">
        <h1>Session ended by staff</h1>
        <p className="error-detail">
          Please save your work and step away. This station will lock in
        </p>
        <p className="hud-timer-value">{formatGrace(graceLeft)}</p>
      </section>
    );
  }

  return (
    <section className="panel session panel-wide">
      <div className="session-header">
        <h1>Welcome, {playerName}</h1>
        {typeof remaining === 'number' ? <HudTimer remainingMinutes={remaining} /> : null}
      </div>

      {!online ? (
        <div className="maintenance-banner" role="alert">
          <p className="error-headline">Connection lost</p>
          <p className="error-detail">
            Reconnecting… your time keeps counting. The station will lock in{' '}
            {formatGrace(offlineLeft ?? OFFLINE_GRACE_MS)} if the connection doesn't return.
          </p>
        </div>
      ) : null}

      <LauncherGrid onError={(m) => pushToast(m, 'warning')} />

      <p className="meta">Realtime: {wsConnected ? 'connected' : 'reconnecting…'}</p>

      {confirmEnd ? (
        <div className="confirm-end" role="alertdialog">
          <p>End your session now? Unsaved game progress may be lost.</p>
          <div className="confirm-end-actions">
            <button type="button" className="danger" onClick={() => void endSession('voluntary')}>
              Yes, end session
            </button>
            <button type="button" className="secondary" onClick={() => setConfirmEnd(false)}>
              Keep playing
            </button>
          </div>
        </div>
      ) : (
        <button type="button" className="secondary" onClick={() => setConfirmEnd(true)}>
          End session
        </button>
      )}

      <ToastHost toasts={toasts} onDismiss={dismissToast} />
    </section>
  );
}
