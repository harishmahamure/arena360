import { SESSION_CLOCK_TICK_MS } from '@gaming-cafe/contracts';
import {
  AUTO_END_REMAINING_SECONDS,
  useAsyncAction,
  useSessionRemainingMinutes,
} from '@gaming-cafe/utils';
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
    online,
    startSession,
    endSession,
    reconcileSession,
  } = useKiosk();

  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [view, setView] = useState<SessionView>('home');
  const [libraryQuery, setLibraryQuery] = useState('');
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [refreshingTime, setRefreshingTime] = useState(false);
  const {
    loading: endingSession,
    succeeded: endSucceeded,
    failed: endFailed,
    errorMessage: endErrorMessage,
    disabled: endDisabled,
    run: runEndSession,
    reset: resetEndSession,
  } = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const [offlineLeft, setOfflineLeft] = useState<number | null>(null);
  const offlineSinceRef = useRef<number | null>(null);
  const startedRef = useRef(false);
  const firedRemindersRef = useRef<Set<number>>(new Set());
  const prevWalletRef = useRef<number | null>(null);
  const autoEndFiredRef = useRef(false);

  const pushToast = useCallback((text: string, tone: ToastMessage['tone'] = 'info') => {
    setToasts((prev) => [...prev, { id: `${Date.now()}-${Math.random()}`, text, tone }]);
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const onError = useCallback((m: string) => pushToast(m, 'warning'), [pushToast]);

  const { processes, closing, closeAll, refresh } = useTrackedProcesses({
    enabled: Boolean(activeSession),
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

  // Create (or resume) the session once when arriving without one.
  useEffect(() => {
    if (!activeSession && !startedRef.current) {
      startedRef.current = true;
      void startSession();
    }
  }, [activeSession, startSession]);

  useEffect(() => {
    if (!activeSession?.id) {
      autoEndFiredRef.current = false;
      prevWalletRef.current = null;
      return;
    }
    autoEndFiredRef.current = false;
    prevWalletRef.current = null;
  }, [activeSession?.id]);

  const walletBalance = activeSession?.walletBalanceMinutes;
  const displayRemaining = useSessionRemainingMinutes(
    activeSession
      ? {
          sessionStartTime: activeSession.startTime,
          walletBalanceMinutes: activeSession.walletBalanceMinutes,
          timeCreditsConsumed: activeSession.timeCreditsConsumed ?? 0,
          deductionProfile: activeSession.deductionProfile,
          cafeTimezone: activeSession.cafeTimezone,
          expiryDate: activeSession.expiryDate ?? undefined,
        }
      : undefined,
  );

  // Recharge toast when wallet balance increases (mid-session top-up via balance.updated).
  useEffect(() => {
    if (typeof walletBalance !== 'number') return;
    const prev = prevWalletRef.current;
    if (prev !== null && walletBalance > prev + 0.05) {
      pushToast('Time added — thanks!', 'success');
      for (const t of REMINDER_THRESHOLDS) {
        if ((displayRemaining ?? walletBalance) > t) firedRemindersRef.current.delete(t);
      }
    }
    prevWalletRef.current = walletBalance;
  }, [walletBalance, displayRemaining, pushToast]);

  // Time-based reminders: driven by the local countdown so sounds fire on schedule.
  useEffect(() => {
    if (typeof displayRemaining !== 'number') return;
    for (const t of REMINDER_THRESHOLDS) {
      if (displayRemaining <= t && !firedRemindersRef.current.has(t)) {
        firedRemindersRef.current.add(t);
        pushToast(`${t} minute${t === 1 ? '' : 's'} remaining`, t === 1 ? 'warning' : 'info');
        if (t !== 1) playRemainingTimeSound(t);
      }
    }
  }, [displayRemaining, pushToast]);

  // Auto-end when display reaches the final seconds threshold (once per session).
  useEffect(() => {
    if (typeof displayRemaining !== 'number' || autoEndFiredRef.current) return;
    const remainingSeconds = Math.floor(displayRemaining * 60);
    if (remainingSeconds <= AUTO_END_REMAINING_SECONDS) {
      autoEndFiredRef.current = true;
      void endSession('auto');
    }
  }, [displayRemaining, endSession]);

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
    const id = setInterval(tick, SESSION_CLOCK_TICK_MS);
    return () => clearInterval(id);
  }, [online, endSession]);

  return (
    <div className="a360-session">
      <SessionNav
        playerName={playerName}
        remainingMinutes={displayRemaining ?? undefined}
        deductionProfile={activeSession?.deductionProfile}
        cafeTimezone={activeSession?.cafeTimezone}
        deviceName={deviceName}
        activeView={view}
        onNavigate={setView}
        onEndSession={() => setConfirmEnd(true)}
        onRefreshTime={async () => {
          setRefreshingTime(true);
          try {
            await reconcileSession();
          } finally {
            setRefreshingTime(false);
          }
        }}
        refreshing={refreshingTime}
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
            deviceName={deviceName}
            onError={onError}
            onLaunched={refresh}
            onNavigate={setView}
            onSearchLibrary={setLibraryQuery}
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
