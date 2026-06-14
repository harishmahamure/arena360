import { useAsyncAction, useSessionRemainingMinutes } from '@gaming-cafe/utils';
import { useCallback, useEffect, useRef, useState } from 'react';
import { AsyncActionButton } from '../components/AsyncActionButton';
import { HomeView } from '../components/session/HomeView';
import { LibraryView } from '../components/session/LibraryView';
import { RunningAppsBar } from '../components/session/RunningAppsBar';
import { SessionNav, type SessionView } from '../components/session/SessionNav';
import { SettingsView } from '../components/session/SettingsView';
import { useTrackedProcesses } from '../components/session/useTrackedProcesses';
import { ToastHost, type ToastMessage } from '../components/Toast';
import { useKiosk } from '../context/KioskProvider';
import { useSessionPoller } from '../hooks/useSessionPoller';
import { OFFLINE_GRACE_MS } from '../lib/config';
import { playRemainingTimeSound } from '../lib/sessionSounds';

const REMINDER_THRESHOLDS = [10, 5, 2, 1] as const;

function formatGrace(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function SessionPage() {
  const {
    playerName,
    deviceName,
    activeSession,
    wsConnected,
    online,
    forceEndGraceEndsAt,
    startSession,
    endSession,
    syncSession,
    heartbeatSession,
  } = useKiosk();

  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [view, setView] = useState<SessionView>('home');
  const [libraryQuery, setLibraryQuery] = useState('');
  const [confirmEnd, setConfirmEnd] = useState(false);
  const {
    loading: endingSession,
    succeeded: endSucceeded,
    failed: endFailed,
    errorMessage: endErrorMessage,
    disabled: endDisabled,
    run: runEndSession,
    reset: resetEndSession,
  } = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const [refreshing, setRefreshing] = useState(false);
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

  const onError = useCallback((m: string) => pushToast(m, 'warning'), [pushToast]);

  const handleRefreshTime = useCallback(async () => {
    setRefreshing(true);
    try {
      await syncSession();
    } catch {
      pushToast('Could not refresh remaining time', 'warning');
    } finally {
      setRefreshing(false);
    }
  }, [pushToast, syncSession]);

  const { processes, closing, closeAll, refresh } = useTrackedProcesses({
    enabled: Boolean(activeSession) && forceEndGraceEndsAt === null,
    onError,
  });

  // "/" focuses library search when not typing in an input.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return;
      const target = e.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT')
      ) {
        return;
      }
      e.preventDefault();
      setView('library');
      requestAnimationFrame(() => {
        const input = document.querySelector<HTMLInputElement>('[data-library-search]');
        input?.focus();
      });
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  // When the player returns from a launched game, land on the session homepage.
  useEffect(() => {
    if (!activeSession || processes.length === 0) return;
    const onFocus = () => setView('home');
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [activeSession, processes.length]);

  // Create (or resume) the session once when arriving without one.
  useEffect(() => {
    if (!activeSession && !startedRef.current) {
      startedRef.current = true;
      void startSession();
    }
  }, [activeSession, startSession]);

  const serverRemaining = activeSession?.remainingMinutes;
  const localRemaining = useSessionRemainingMinutes(
    serverRemaining,
    activeSession?.deductionProfile,
    activeSession?.cafeTimezone,
  );
  const activeSessionId = activeSession?.id;
  useSessionPoller(
    localRemaining ?? serverRemaining,
    syncSession,
    Boolean(activeSession) && forceEndGraceEndsAt === null,
    wsConnected ? 60_000 : undefined,
  );

  useEffect(() => {
    if (!activeSessionId || forceEndGraceEndsAt !== null) return;
    const id = setInterval(() => {
      void heartbeatSession();
    }, 120_000);
    return () => clearInterval(id);
  }, [activeSessionId, forceEndGraceEndsAt, heartbeatSession]);

  // Recharge toast when the server-authoritative value increases (mid-session top-up).
  useEffect(() => {
    if (typeof serverRemaining !== 'number') return;
    const prev = prevMinutesRef.current;
    if (prev !== null && serverRemaining > prev + 0.05) {
      pushToast('Time added — thanks!', 'success');
      for (const t of REMINDER_THRESHOLDS) {
        if (serverRemaining > t) firedRemindersRef.current.delete(t);
      }
    }
    prevMinutesRef.current = serverRemaining;
  }, [serverRemaining, pushToast]);

  // Time-based reminders: driven by the local countdown so sounds fire on schedule.
  useEffect(() => {
    if (typeof localRemaining !== 'number') return;
    for (const t of REMINDER_THRESHOLDS) {
      if (localRemaining <= t && !firedRemindersRef.current.has(t)) {
        firedRemindersRef.current.add(t);
        pushToast(`${t} minute${t === 1 ? '' : 's'} remaining`, t === 1 ? 'warning' : 'info');
        if (t !== 1) playRemainingTimeSound(t);
      }
    }
  }, [localRemaining, pushToast]);

  // Auto-end when the local countdown reaches zero (unless extended in time).
  useEffect(() => {
    if (typeof localRemaining === 'number' && localRemaining <= 0 && forceEndGraceEndsAt === null) {
      void endSession('auto');
    }
  }, [localRemaining, forceEndGraceEndsAt, endSession]);

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
    <div className="a360-session">
      <SessionNav
        playerName={playerName}
        remainingMinutes={localRemaining ?? serverRemaining}
        deductionProfile={activeSession?.deductionProfile}
        cafeTimezone={activeSession?.cafeTimezone}
        deviceName={deviceName}
        activeView={view}
        onNavigate={setView}
        onEndSession={() => setConfirmEnd(true)}
        onRefreshTime={handleRefreshTime}
        refreshing={refreshing}
        onError={onError}
      />

      <RunningAppsBar processes={processes} closing={closing} onCloseAll={closeAll} />

      <div className={`a360-session-body${view === 'home' ? ' a360-session-body--home' : ''}`}>
        {!online ? (
          <div className="a360-section">
            <div className="maintenance-banner" role="alert">
              <p className="error-headline">Connection lost</p>
              <p className="error-detail">
                Reconnecting… your time keeps counting. The station will lock in{' '}
                {formatGrace(offlineLeft ?? OFFLINE_GRACE_MS)} if the connection doesn't return.
              </p>
            </div>
          </div>
        ) : null}

        {view === 'home' ? (
          <HomeView
            onError={onError}
            onNavigate={setView}
            onSearchLibrary={setLibraryQuery}
            onLaunched={refresh}
          />
        ) : view === 'library' ? (
          <LibraryView initialQuery={libraryQuery} onError={onError} onLaunched={refresh} />
        ) : (
          <SettingsView onError={onError} onLaunched={refresh} />
        )}
      </div>

      {confirmEnd ? (
        <div className="a360-modal-scrim">
          <div className="confirm-end glass-card" role="alertdialog" aria-modal="true">
            <p>End your session now? Unsaved game progress may be lost.</p>
            <div className="confirm-end-actions">
              <AsyncActionButton
                className="danger"
                loading={endingSession}
                success={endSucceeded}
                successLabel="Session ended"
                error={endFailed}
                errorLabel={endErrorMessage ?? 'Could not end session'}
                loadingLabel="Ending…"
                disabled={endDisabled}
                onClick={() => {
                  void runEndSession(async () => {
                    await endSession('voluntary');
                    setConfirmEnd(false);
                  });
                }}
              >
                Yes, end session
              </AsyncActionButton>
              <button
                type="button"
                className="secondary"
                disabled={endingSession || endSucceeded}
                onClick={() => {
                  setConfirmEnd(false);
                  resetEndSession();
                }}
              >
                Keep playing
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <ToastHost toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
