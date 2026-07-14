import { SESSION_CLOCK_TICK_MS } from '@gaming-cafe/contracts';
import { AUTO_END_REMAINING_SECONDS, useAsyncAction } from '@gaming-cafe/utils';
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { AsyncActionButton } from '../components/AsyncActionButton';
import { HomeView } from '../components/session/HomeView';
import { LibraryView } from '../components/session/LibraryView';
import { MenuView } from '../components/session/MenuView';
import { RunningAppsBar } from '../components/session/RunningAppsBar';
import { SessionNav, type SessionView } from '../components/session/SessionNav';
import { SettingsView } from '../components/session/SettingsView';
import { useTrackedProcesses } from '../components/session/useTrackedProcesses';
import { ToastHost, type ToastMessage } from '../components/Toast';
import { useKiosk } from '../context/KioskProvider';
import { useSessionTimerController } from '../hooks/useSessionTimerController';
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
    playerRole,
    deviceName,
    activeSession,
    online,
    startSession,
    endSession,
    reconcileSession,
    reauthRequired,
    playerReauth,
    dismissReauth,
  } = useKiosk();

  const confirmEndTitleId = useId();
  const [reauthPassword, setReauthPassword] = useState('');
  const [reauthBusy, setReauthBusy] = useState(false);
  const [reauthError, setReauthError] = useState<string | null>(null);

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
  const timerInput = activeSession
    ? {
        sessionStartTime: activeSession.startTime,
        walletBalanceMinutes: activeSession.walletBalanceMinutes,
        timeCreditsConsumed: activeSession.timeCreditsConsumed ?? 0,
        deductionProfile: activeSession.deductionProfile,
        cafeTimezone: activeSession.cafeTimezone,
        expiryDate: activeSession.expiryDate ?? undefined,
      }
    : undefined;

  useSessionTimerController(timerInput, (displayRemaining) => {
    if (typeof displayRemaining !== 'number') return;

    for (const t of REMINDER_THRESHOLDS) {
      if (displayRemaining <= t && !firedRemindersRef.current.has(t)) {
        firedRemindersRef.current.add(t);
        pushToast(`${t} minute${t === 1 ? '' : 's'} remaining`, t === 1 ? 'warning' : 'info');
        if (t !== 1) playRemainingTimeSound(t);
      }
    }

    if (!autoEndFiredRef.current) {
      const remainingSeconds = Math.floor(displayRemaining * 60);
      if (remainingSeconds <= AUTO_END_REMAINING_SECONDS) {
        autoEndFiredRef.current = true;
        void endSession('auto');
      }
    }
  });

  // Recharge toast when wallet balance increases (mid-session top-up via balance.updated).
  useEffect(() => {
    if (typeof walletBalance !== 'number') return;
    const prev = prevWalletRef.current;
    if (prev !== null && walletBalance > prev + 0.05) {
      pushToast('Time added — thanks!', 'success');
      for (const t of REMINDER_THRESHOLDS) {
        firedRemindersRef.current.delete(t);
      }
    }
    prevWalletRef.current = walletBalance;
  }, [walletBalance, pushToast]);

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
        playerRole={playerRole}
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
            onError={onError}
            onLaunched={refresh}
            onNavigate={setView}
            onSearchLibrary={setLibraryQuery}
          />
        ) : view === 'library' ? (
          <LibraryView initialQuery={libraryQuery} onError={onError} onLaunched={refresh} />
        ) : view === 'menu' ? (
          <MenuView onError={onError} />
        ) : (
          <SettingsView onError={onError} onLaunched={refresh} />
        )}
      </div>

      {confirmEnd ? (
        <div className="a360-modal-scrim">
          <div
            className="confirm-end glass-card"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby={confirmEndTitleId}
          >
            <p id={confirmEndTitleId}>End your session now? Unsaved game progress may be lost.</p>
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
                  resetEndSession();
                  setConfirmEnd(false);
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {reauthRequired ? (
        <div className="a360-modal-scrim">
          <div
            className="confirm-end glass-card"
            role="dialog"
            aria-modal="true"
            aria-labelledby="reauth-title"
          >
            <h2 id="reauth-title">Session paused — sign in again</h2>
            <p className="hint">
              Your session timed out for security. Re-enter your password to continue playing as{' '}
              {playerName ?? 'your account'}.
            </p>
            <form
              className="a360-form"
              onSubmit={(e) => {
                e.preventDefault();
                setReauthBusy(true);
                setReauthError(null);
                void playerReauth(reauthPassword)
                  .then(() => setReauthPassword(''))
                  .catch((err) => {
                    setReauthError(err instanceof Error ? err.message : 'Re-authentication failed');
                  })
                  .finally(() => setReauthBusy(false));
              }}
            >
              <label className="a360-field" htmlFor="reauth-password">
                Password
                <input
                  id="reauth-password"
                  type="password"
                  value={reauthPassword}
                  onChange={(e) => setReauthPassword(e.target.value)}
                  required
                  autoComplete="current-password"
                />
              </label>
              {reauthError ? <p className="error">{reauthError}</p> : null}
              <div className="confirm-end-actions">
                <button type="submit" className="primary-glow-btn" disabled={reauthBusy}>
                  {reauthBusy ? 'Signing in…' : 'Continue session'}
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={reauthBusy}
                  onClick={() => {
                    setReauthPassword('');
                    setReauthError(null);
                    dismissReauth();
                    void endSession('auth_expired');
                  }}
                >
                  End session
                </button>
              </div>
            </form>
          </div>
        </div>
      ) : null}

      <ToastHost toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
