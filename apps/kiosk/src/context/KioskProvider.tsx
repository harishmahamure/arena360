import type { DeductionProfile } from '@gaming-cafe/contracts';
import { ApiError } from '@gaming-cafe/utils';
import { listen } from '@tauri-apps/api/event';
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { SESSION_EXPIRED_MESSAGE } from '../lib/authMessages';
import { isTokenAuthFailure, registerAuthSessionHandlers } from '../lib/authSession';
import { applyBalanceUpdated, mergeReconciledSession } from '../lib/balanceUpdated';
import { SESSION_RECONCILE_MS } from '../lib/config';
import {
  clearPlayerSession,
  getHttpClient,
  loadTokensIntoCache,
  persistDeviceToken,
  persistPlayerToken,
  tokenCache,
} from '../lib/http';
import { resetLoginLockoutByStaff } from '../lib/loginLockout';
import { enqueueEndIntent, loadEndIntents, removeEndIntent } from '../lib/offlineQueue';
import { formatPlayerLoginError } from '../lib/planErrors';
import { KioskRealtimeClient } from '../lib/realtime';
import {
  isRemoteSessionEndEvent,
  type SessionEndPayload,
  shouldEndSessionForRemoteEvent,
  shouldRunWsFailPoll,
  staffEndedFromPayload,
} from '../lib/remoteSessionEnd';
import {
  clearPlayerPersistedState,
  readSessionSnapshot,
  readStoredPlayerName,
  storePlayerName,
  storeSessionSnapshot,
} from '../lib/sessionPersist';
import { prepareSessionSounds } from '../lib/sessionSounds';
import { resolveSetupShortcutAction } from '../lib/setupShortcut';
import {
  clearAllTokens,
  focusKiosk,
  killTrackedProcesses,
  setLockdownState,
} from '../lib/tauriCommands';

export type AppPhase =
  | 'loading'
  | 'register'
  | 'login'
  | 'setup'
  | 'session'
  | 'already-in-session';

interface KioskSessionResponse {
  sessionId: string;
  balanceId: string;
  deviceId: string;
  startTime: string;
  remainingMinutes: number;
  walletBalanceMinutes?: number;
  resumed: boolean;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string;
}

interface LoginActiveSessionResponse {
  id: string;
  startTime: string;
  balanceId: string;
  remainingMinutes?: number;
  walletBalanceMinutes?: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string;
}

export interface ActiveSession {
  id: string;
  startTime: string;
  balanceId: string;
  walletBalanceMinutes: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string | null;
}

function walletMinutesFromResponse(
  walletBalanceMinutes: number | undefined,
  remainingMinutes: number | undefined,
): number {
  if (typeof walletBalanceMinutes === 'number') return walletBalanceMinutes;
  if (typeof remainingMinutes === 'number') return remainingMinutes;
  return 0;
}

function sessionFromResponse(res: KioskSessionResponse): ActiveSession {
  return {
    id: res.sessionId,
    startTime: res.startTime,
    balanceId: res.balanceId,
    walletBalanceMinutes: walletMinutesFromResponse(res.walletBalanceMinutes, res.remainingMinutes),
    deductionProfile: res.deductionProfile ?? null,
    cafeTimezone: res.cafeTimezone,
    timeCreditsConsumed: res.timeCreditsConsumed ?? null,
    expiryDate: res.expiryDate ?? null,
  };
}

function activeSessionFromLogin(raw: LoginActiveSessionResponse): ActiveSession {
  return {
    id: raw.id,
    startTime: raw.startTime,
    balanceId: raw.balanceId,
    walletBalanceMinutes: walletMinutesFromResponse(raw.walletBalanceMinutes, raw.remainingMinutes),
    deductionProfile: raw.deductionProfile ?? null,
    cafeTimezone: raw.cafeTimezone,
    timeCreditsConsumed: raw.timeCreditsConsumed ?? null,
    expiryDate: raw.expiryDate ?? null,
  };
}

export interface DeviceProvisionInput {
  name: string;
  deviceType: string;
  deviceSubType: string;
  location?: string;
  serialNumber?: string;
}

interface KioskContextValue {
  phase: AppPhase;
  deviceId: string | null;
  deviceName: string | null;
  playerName: string | null;
  playerRole: string | null;
  activeSession: ActiveSession | null;
  wsConnected: boolean;
  lastEvent: string | null;
  error: string | null;
  /** Device status string from the latest `device.status_changed` event. */
  deviceStatus: string | null;
  /** True when the device is under maintenance / out of service (blocks login). */
  maintenance: boolean;
  /** Name of the station holding the player's open session (single-login guard). */
  conflictDevice: string | null;
  /** False when the backend is unreachable; drives offline UX. */
  online: boolean;
  /** Non-blocking notice shown on the login screen (e.g. staff force-end). */
  loginNotice: string | null;
  clearLoginNotice: () => void;
  clearError: () => void;
  refresh: () => Promise<void>;
  /** First-time provisioning: register this device using the captured admin token. */
  provisionDevice: (input: DeviceProvisionInput) => Promise<void>;
  /** True once an admin has signed in during registration. */
  adminAuthenticated: boolean;
  /** True after registration until SetupPage consumes the handoff (skip re-login). */
  setupAuthenticated: boolean;
  clearSetupAuthenticated: () => void;
  enterSetup: () => Promise<void>;
  exitSetup: () => Promise<void>;
  /** Admin sign-in (optional TOTP). Stores token; optionally relaxes lockdown for setup. */
  adminLogin: (
    username: string,
    password: string,
    totp?: string,
    options?: { relaxLockdown?: boolean },
  ) => Promise<void>;
  playerLogin: (username: string, password: string) => Promise<void>;
  playerLogout: () => Promise<void>;
  /** Start (or resume) the kiosk session for the signed-in player. */
  startSession: () => Promise<void>;
  /** Resync session clock fields from GET /kiosk/sessions/current. */
  reconcileSession: () => Promise<void>;
  /** End the current session with a reason and run process cleanup. */
  endSession: (reason?: string) => Promise<void>;
  /** Dismiss the single-login conflict screen back to idle. */
  dismissConflict: () => void;
  factoryReset: () => Promise<void>;
  /** Bumped when staff clears player login lockout (Ctrl+Shift+B). */
  staffLockoutClearTick: number;
}

const KioskContext = createContext<KioskContextValue | null>(null);

const DEVICE_NAME_KEY = 'gaming-cafe.kiosk.device_name';

function readStoredDeviceName(): string | null {
  try {
    return localStorage.getItem(DEVICE_NAME_KEY);
  } catch {
    return null;
  }
}

function storeDeviceName(name: string | null): void {
  try {
    if (name) localStorage.setItem(DEVICE_NAME_KEY, name);
    else localStorage.removeItem(DEVICE_NAME_KEY);
  } catch {
    // localStorage unavailable — non-fatal
  }
}

function toErrorMessage(e: unknown, fallback: string): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error && e.message) return e.message;
  return fallback;
}

function playerIdFromToken(token: string): string | null {
  try {
    const payload = JSON.parse(atob(token.split('.')[1] ?? '')) as { userId?: string };
    return typeof payload.userId === 'string' ? payload.userId : null;
  } catch {
    return null;
  }
}

export function KioskProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<AppPhase>('loading');
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [playerName, setPlayerName] = useState<string | null>(null);
  const [playerRole, setPlayerRole] = useState<string | null>(null);
  const [activeSession, setActiveSession] = useState<ActiveSession | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const [lastEvent, setLastEvent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deviceStatus, setDeviceStatus] = useState<string | null>(null);
  const [conflictDevice, setConflictDevice] = useState<string | null>(null);
  const [online, setOnline] = useState(true);
  const [loginNotice, setLoginNotice] = useState<string | null>(null);
  const deviceIdRef = useRef<string | null>(null);
  const phaseRef = useRef<AppPhase>('loading');
  const setupRelaxedRef = useRef(false);
  const activeSessionRef = useRef<ActiveSession | null>(null);
  const cleanupInFlightRef = useRef(false);
  const lastEnterSetupAtRef = useRef(0);
  const lastClearLockoutAtRef = useRef(0);
  const SETUP_SHORTCUT_DEBOUNCE_MS = 400;
  // Short-lived admin token captured during first-time provisioning (in memory only).
  const [adminToken, setAdminToken] = useState<string | null>(null);
  const [setupAuthenticated, setSetupAuthenticated] = useState(false);
  const [staffLockoutClearTick, setStaffLockoutClearTick] = useState(0);

  const maintenance = deviceStatus === 'under_maintenance' || deviceStatus === 'out_of_service';

  const realtimeRef = useMemo(() => ({ current: new KioskRealtimeClient() }), []);
  const handleRemoteSessionEndRef = useRef<
    ((opts?: { staffEnded?: boolean }) => Promise<void>) | null
  >(null);

  useEffect(() => {
    deviceIdRef.current = deviceId;
  }, [deviceId]);

  useEffect(() => {
    phaseRef.current = phase;
  }, [phase]);

  useEffect(() => {
    activeSessionRef.current = activeSession;
  }, [activeSession]);

  useEffect(() => {
    if (activeSession) {
      storeSessionSnapshot(activeSession);
    }
  }, [activeSession]);

  useEffect(() => {
    const syncOnline = () => setOnline(navigator.onLine);
    syncOnline();
    window.addEventListener('online', syncOnline);
    window.addEventListener('offline', syncOnline);
    return () => {
      window.removeEventListener('online', syncOnline);
      window.removeEventListener('offline', syncOnline);
    };
  }, []);

  const connectWs = useCallback(
    (id: string, playerId?: string) => {
      const rt = realtimeRef.current;
      const channels = [`device:${id}`];
      if (playerId) channels.push(`user:${playerId}`);
      rt.resetSubscriptions(channels);
      rt.disconnect();
      rt.connect();
    },
    [realtimeRef],
  );

  const flushEndIntents = useCallback(async () => {
    const intents = loadEndIntents();
    if (intents.length === 0) return;
    const http = getHttpClient();
    for (const intent of intents) {
      try {
        // Idempotent: ending an already-closed session is a no-op server-side.
        await http.patch(`/kiosk/sessions/${intent.sessionId}/end`, {
          reason: intent.reason,
        });
        removeEndIntent(intent.sessionId);
      } catch {
        // Still offline / transient — keep the intent for the next attempt.
        break;
      }
    }
  }, []);

  const cleanupSessionAndReturnToLogin = useCallback(
    async (opts?: { staffEnded?: boolean }) => {
      try {
        await killTrackedProcesses();
      } catch {
        // Process cleanup is best-effort off-Windows / when nothing tracked.
      }
      setActiveSession(null);
      await clearPlayerSession();
      setPlayerName(null);
      setPlayerRole(null);
      clearPlayerPersistedState();
      if (opts?.staffEnded) {
        setLoginNotice('Your session was ended by staff.');
      }
      setPhase('login');
      const id = deviceIdRef.current;
      if (id) connectWs(id);
    },
    [connectWs],
  );

  const handleRemoteSessionEnd = useCallback(
    async (opts?: { staffEnded?: boolean }) => {
      if (cleanupInFlightRef.current) return;
      cleanupInFlightRef.current = true;
      try {
        await cleanupSessionAndReturnToLogin(opts);
      } finally {
        cleanupInFlightRef.current = false;
      }
    },
    [cleanupSessionAndReturnToLogin],
  );

  const reconcileSessionOnce = useCallback(
    async (opts?: { staffEndedOnRemoteEnd?: boolean }) => {
      if (phaseRef.current !== 'session' || !tokenCache.player) return;
      try {
        const res = await getHttpClient().get<KioskSessionResponse | null>(
          '/kiosk/sessions/current',
        );
        setOnline(true);
        await flushEndIntents();
        if (!res) {
          await handleRemoteSessionEnd({ staffEnded: opts?.staffEndedOnRemoteEnd ?? false });
          return;
        }
        setActiveSession((prev) => mergeReconciledSession(sessionFromResponse(res), prev));
      } catch {
        setOnline(false);
      }
    },
    [flushEndIntents, handleRemoteSessionEnd],
  );

  useEffect(() => {
    handleRemoteSessionEndRef.current = handleRemoteSessionEnd;
  }, [handleRemoteSessionEnd]);

  useEffect(() => {
    const rt = realtimeRef.current;
    return rt.onAny((frame) => {
      if (frame.event_type) setLastEvent(frame.event_type);

      if (isRemoteSessionEndEvent(frame.event_type ?? '')) {
        const payload = frame.payload as SessionEndPayload | undefined;
        if (!shouldEndSessionForRemoteEvent(activeSessionRef.current?.id, payload)) {
          return;
        }
        void handleRemoteSessionEndRef.current?.({
          staffEnded: staffEndedFromPayload(frame.event_type ?? '', payload),
        });
        return;
      }

      if (frame.event_type === 'device.status_changed') {
        const payload = frame.payload as { status?: string } | undefined;
        if (typeof payload?.status === 'string') {
          setDeviceStatus(payload.status);
          if (payload.status === 'under_maintenance' || payload.status === 'out_of_service') {
            setPhase((prev) => (prev === 'login' ? 'login' : prev));
          }
        }
      }

      if (frame.event_type === 'balance.updated') {
        setActiveSession((prev) => {
          const updated = applyBalanceUpdated(prev, frame.payload);
          return updated ?? prev;
        });
      }
    });
  }, [realtimeRef]);

  useEffect(() => {
    const rt = realtimeRef.current;
    const interval = setInterval(() => setWsConnected(rt.connected), 500);
    return () => clearInterval(interval);
  }, [realtimeRef]);

  useEffect(() => {
    const rt = realtimeRef.current;
    return rt.onConnect(() => {
      if (phaseRef.current === 'session' && tokenCache.player) {
        void reconcileSessionOnce();
      }
    });
  }, [realtimeRef, reconcileSessionOnce]);

  useEffect(() => {
    const onResume = () => {
      if (document.visibilityState !== 'visible') return;
      if (phaseRef.current === 'session' && tokenCache.player) {
        void reconcileSessionOnce();
      }
    };
    document.addEventListener('visibilitychange', onResume);
    window.addEventListener('focus', onResume);
    return () => {
      document.removeEventListener('visibilitychange', onResume);
      window.removeEventListener('focus', onResume);
    };
  }, [reconcileSessionOnce]);

  // Poll for remote session ends only while WebSocket is disconnected (5 min default).
  useEffect(() => {
    if (!shouldRunWsFailPoll(phase, wsConnected, Boolean(tokenCache.player))) {
      return;
    }
    const id = setInterval(() => {
      void reconcileSessionOnce({ staffEndedOnRemoteEnd: true });
    }, SESSION_RECONCILE_MS);
    return () => clearInterval(id);
  }, [phase, wsConnected, reconcileSessionOnce]);

  const handlePlayerAuthExpired = useCallback(async () => {
    await cleanupSessionAndReturnToLogin();

    setConflictDevice(null);
    setError(SESSION_EXPIRED_MESSAGE);
  }, [cleanupSessionAndReturnToLogin]);

  const handleDeviceAuthExpired = useCallback(async () => {
    if (phaseRef.current === 'session') {
      try {
        await killTrackedProcesses();
      } catch {
        // Process cleanup is best-effort off-Windows / when nothing tracked.
      }
    }
    await clearAllTokens();
    tokenCache.device = undefined;
    tokenCache.player = undefined;
    realtimeRef.current.disconnect();
    setAdminToken(null);
    setDeviceId(null);
    setDeviceName(null);
    storeDeviceName(null);
    clearPlayerPersistedState();
    setPlayerName(null);
    setPlayerRole(null);
    setActiveSession(null);
    setConflictDevice(null);
    setPhase('register');
    setError('This station needs to be re-registered.');
    await setLockdownState('Locked');
  }, [realtimeRef]);

  useEffect(() => {
    registerAuthSessionHandlers({
      getRuntime: () => ({
        phase,
        hasPlayerToken: Boolean(tokenCache.player),
      }),
      onPlayerLogout: handlePlayerAuthExpired,
      onFullReset: handleDeviceAuthExpired,
    });
  }, [handleDeviceAuthExpired, handlePlayerAuthExpired, phase]);

  const tryRestorePlayerSession = useCallback(
    async (deviceId: string): Promise<boolean> => {
      if (!tokenCache.player) return false;

      const playerId = playerIdFromToken(tokenCache.player);
      if (!playerId) {
        await clearPlayerSession();
        clearPlayerPersistedState();
        return false;
      }

      setPlayerName(readStoredPlayerName());

      const http = getHttpClient();
      try {
        const res = await http.get<KioskSessionResponse | null>('/kiosk/sessions/current');
        setOnline(true);
        await flushEndIntents();
        if (!res) {
          await clearPlayerSession();
          clearPlayerPersistedState();
          return false;
        }
        setActiveSession(sessionFromResponse(res));
        prepareSessionSounds();
        setPhase('session');
        connectWs(deviceId, playerId);
        return true;
      } catch (e) {
        if (e instanceof ApiError && isTokenAuthFailure(e.message)) {
          await clearPlayerSession();
          clearPlayerPersistedState();
          return false;
        }

        const cached = readSessionSnapshot();
        if (cached) {
          setActiveSession(cached);
          prepareSessionSounds();
          setPhase('session');
          setOnline(false);
          connectWs(deviceId, playerId);
          return true;
        }
        return false;
      }
    },
    [connectWs, flushEndIntents],
  );

  const refresh = useCallback(async () => {
    setError(null);
    await loadTokensIntoCache();
    if (!tokenCache.device) {
      setPhase('register');
      await setLockdownState('Locked');
      return;
    }
    setDeviceName(readStoredDeviceName());
    let deviceId: string | null = null;
    try {
      const payload = JSON.parse(atob(tokenCache.device.split('.')[1] ?? ''));
      deviceId = payload.userId as string;
      setDeviceId(deviceId);
    } catch {
      setDeviceId(null);
    }

    if (tokenCache.player && deviceId) {
      const restored = await tryRestorePlayerSession(deviceId);
      if (restored) {
        await setLockdownState('Locked');
        return;
      }
    } else if (deviceId) {
      connectWs(deviceId);
    }

    setPhase('login');
    await setLockdownState('Locked');
  }, [connectWs, tryRestorePlayerSession]);

  useEffect(() => {
    void refresh();
    return () => realtimeRef.current.disconnect();
  }, [refresh, realtimeRef]);

  // Reconcile the React phase with the native lockdown state. When setup mode
  // ends (SetupRelaxed → Locked), return to the player login screen. Do not
  // bounce when opening setup admin login via Ctrl+Shift+A while still Locked.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/event').then(({ listen }) =>
      listen<string>('lockdown-changed', (event) => {
        if (event.payload === 'SetupRelaxed') {
          setupRelaxedRef.current = true;
        } else if (event.payload === 'Locked' && setupRelaxedRef.current) {
          setupRelaxedRef.current = false;
          setPhase((prev) => (prev === 'setup' ? 'login' : prev));
        }
      }).then((fn) => {
        unlisten = fn;
      }),
    );
    return () => unlisten?.();
  }, []);

  const adminLogin = useCallback(
    async (
      username: string,
      password: string,
      totp?: string,
      options?: { relaxLockdown?: boolean },
    ) => {
      setError(null);
      try {
        const http = getHttpClient();
        const res = await http.post<{ accessToken: string }>('/auth/login/admin', {
          username,
          password,
          totp,
        });
        setAdminToken(res.accessToken);
        if (options?.relaxLockdown) {
          await setLockdownState('SetupRelaxed');
        }
      } catch (e) {
        setError(toErrorMessage(e, 'Admin sign-in failed'));
        throw e;
      }
    },
    [],
  );

  const provisionDevice = useCallback(
    async (input: DeviceProvisionInput) => {
      setError(null);
      if (!adminToken) {
        setError('Administrator sign-in is required before registering this device.');
        throw new Error('admin token missing');
      }
      try {
        const http = getHttpClient();
        const fingerprint = await import('../lib/tauriCommands').then((m) =>
          m.collectFingerprint(),
        );
        // The admin token authorizes the privileged provision call (DRAFT-0023).
        // Provisioning runs before a device token exists, so seed the bearer with
        // the admin token for this one request.
        tokenCache.device = adminToken;
        const result = await http.post<{
          accessToken: string;
          device: { id: string; name: string };
        }>('/devices/provision', {
          fingerprint,
          name: input.name,
          deviceType: input.deviceType,
          deviceSubType: input.deviceSubType,
          location: input.location || undefined,
          serialNumber: input.serialNumber || fingerprint.mac,
        });
        await persistDeviceToken(result.accessToken);
        setDeviceId(result.device.id);
        setDeviceName(result.device.name);
        storeDeviceName(result.device.name);
        setAdminToken(null);
        await setLockdownState('SetupRelaxed');
        setSetupAuthenticated(true);
        setPhase('setup');
        connectWs(result.device.id);
      } catch (e) {
        // Revert the temporary admin bearer so a failed attempt leaves no token.
        tokenCache.device = undefined;
        setError(toErrorMessage(e, 'Device registration failed'));
        throw e;
      }
    },
    [adminToken, connectWs],
  );

  const enterSetup = useCallback(async () => {
    setError(null);
    await focusKiosk().catch(() => {
      // Non-fatal when running outside Tauri (browser dev).
    });
    // Stay Locked until an admin authenticates (ADR-0020): the setup gesture
    // only reveals the login form, it does not relax lockdown.
    setPhase('setup');
  }, []);

  const requestEnterSetup = useCallback(() => {
    const now = Date.now();
    if (now - lastEnterSetupAtRef.current < SETUP_SHORTCUT_DEBOUNCE_MS) return;
    lastEnterSetupAtRef.current = now;

    switch (resolveSetupShortcutAction(phaseRef.current)) {
      case 'noop':
        return;
      case 'focusSetup':
        void focusKiosk().catch(() => {
          // Non-fatal when running outside Tauri (browser dev).
        });
        return;
      case 'enterSetup':
        void enterSetup();
        return;
    }
  }, [enterSetup]);

  const requestClearLoginLockout = useCallback(() => {
    const now = Date.now();
    if (now - lastClearLockoutAtRef.current < SETUP_SHORTCUT_DEBOUNCE_MS) return;
    lastClearLockoutAtRef.current = now;

    if (phaseRef.current !== 'login') return;
    resetLoginLockoutByStaff();
    setStaffLockoutClearTick((tick) => tick + 1);
  }, []);

  const clearSetupAuthenticated = useCallback(() => {
    setSetupAuthenticated(false);
  }, []);

  const exitSetup = useCallback(async () => {
    setSetupAuthenticated(false);
    const wasRelaxed = setupRelaxedRef.current;
    setupRelaxedRef.current = false;
    if (wasRelaxed) {
      await setLockdownState('Locked');
    }
    setPhase('login');
  }, []);

  // Setup entry via Ctrl+Shift+A (ADR-0020 amendment). Windows also handles this
  // in the native keyboard hook (`enter-setup` event) when the webview lacks focus.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen('enter-setup', () => {
      requestEnterSetup();
    }).then((fn) => {
      unlisten = fn;
    });

    function onKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey && e.shiftKey && e.code === 'KeyA') {
        e.preventDefault();
        requestEnterSetup();
      }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => {
      unlisten?.();
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [requestEnterSetup]);

  // Staff lockout clear via Ctrl+Shift+B (ADR-0020). Native hook emits
  // `clear-login-lockout` when the webview lacks focus under lockdown.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen('clear-login-lockout', () => {
      requestClearLoginLockout();
    }).then((fn) => {
      unlisten = fn;
    });

    function onKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey && e.shiftKey && e.code === 'KeyB') {
        e.preventDefault();
        requestClearLoginLockout();
      }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => {
      unlisten?.();
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [requestClearLoginLockout]);

  const playerLogin = useCallback(
    async (username: string, password: string) => {
      setError(null);
      if (!online) {
        setError('This station is offline. Please wait for the connection to return.');
        throw new Error('offline');
      }
      const http = getHttpClient();
      try {
        const res = await http.post<{
          accessToken: string;
          user: { id: string; username: string; role: string };
          activeSession?: LoginActiveSessionResponse | null;
        }>('/auth/login/player', { username, password });

        await persistPlayerToken(res.accessToken);
        setPlayerName(res.user.username);
        setPlayerRole(res.user.role);
        storePlayerName(res.user.username);

        if (res.activeSession) {
          setActiveSession(activeSessionFromLogin(res.activeSession));
        } else {
          // Start the session in the same turn as login so X-Player-Token is
          // guaranteed to be in tokenCache before POST /kiosk/sessions (ADR-0017).
          const sessionRes = await http.post<KioskSessionResponse>('/kiosk/sessions', {});
          setActiveSession(sessionFromResponse(sessionRes));
          setConflictDevice(null);
        }

        prepareSessionSounds();
        setPhase('session');
        await setLockdownState('Locked');
        if (deviceId) connectWs(deviceId, res.user.id);
      } catch (e) {
        if (e instanceof ApiError && e.code === 'PLAYER_ALREADY_IN_SESSION') {
          const details = e.details as { deviceName?: string } | undefined;
          setConflictDevice(details?.deviceName ?? 'another station');
          setPhase('already-in-session');
          return;
        }
        await clearPlayerSession();
        if (e instanceof ApiError) {
          setError(formatPlayerLoginError(e.message));
        } else {
          setError('Login failed');
        }
        throw e;
      }
    },
    [connectWs, deviceId, online],
  );

  const playerLogout = useCallback(async () => {
    await clearPlayerSession();
    setPlayerName(null);
    setPlayerRole(null);
    clearPlayerPersistedState();
    setActiveSession(null);
    setPhase('login');
    if (deviceId) connectWs(deviceId);
  }, [connectWs, deviceId]);

  const startSession = useCallback(async () => {
    setError(null);
    if (!tokenCache.player) {
      await loadTokensIntoCache();
    }
    if (!tokenCache.player) {
      setError('Please sign in again.');
      setPhase('login');
      return;
    }
    const http = getHttpClient();
    try {
      const res = await http.post<KioskSessionResponse>('/kiosk/sessions', {});
      setActiveSession(sessionFromResponse(res));
      prepareSessionSounds();
      setConflictDevice(null);
      setPhase('session');
      await setLockdownState('Locked');
    } catch (e) {
      if (e instanceof ApiError && e.code === 'PLAYER_ALREADY_IN_SESSION') {
        const details = e.details as { deviceName?: string } | undefined;
        setConflictDevice(details?.deviceName ?? 'another station');
        setPhase('already-in-session');
        return;
      }
      setError(toErrorMessage(e, 'Could not start the session'));
      throw e;
    }
  }, []);

  const endSessionInFlightRef = useRef(false);
  const factoryResetInFlightRef = useRef(false);

  const endSession = useCallback(
    async (reason = 'voluntary') => {
      if (endSessionInFlightRef.current) return;
      endSessionInFlightRef.current = true;
      try {
        const sessionId = activeSession?.id;
        if (sessionId) {
          try {
            await getHttpClient().patch(`/kiosk/sessions/${sessionId}/end`, { reason });
            setOnline(true);
          } catch {
            // Offline at end time: queue an idempotent replay for reconnect (D18).
            enqueueEndIntent(sessionId, reason === 'voluntary' ? 'offline_reconcile' : reason);
          }
        }
        await cleanupSessionAndReturnToLogin();
      } finally {
        endSessionInFlightRef.current = false;
      }
    },
    [activeSession?.id, cleanupSessionAndReturnToLogin],
  );

  const dismissConflict = useCallback(() => {
    setConflictDevice(null);
    setPlayerName(null);
    setPlayerRole(null);
    clearPlayerPersistedState();
    void clearPlayerSession();
    setPhase('login');
    if (deviceId) connectWs(deviceId);
  }, [connectWs, deviceId]);

  const clearLoginNotice = useCallback(() => setLoginNotice(null), []);
  const clearError = useCallback(() => setError(null), []);

  // Replay any queued end intents whenever connectivity is (re)established.
  useEffect(() => {
    if (online) void flushEndIntents();
  }, [online, flushEndIntents]);

  const factoryReset = useCallback(async () => {
    if (factoryResetInFlightRef.current) return;
    factoryResetInFlightRef.current = true;
    try {
      await clearAllTokens();
      tokenCache.device = undefined;
      tokenCache.player = undefined;
      realtimeRef.current.disconnect();
      setDeviceId(null);
      setDeviceName(null);
      storeDeviceName(null);
      clearPlayerPersistedState();
      setPlayerName(null);
      setPlayerRole(null);
      setActiveSession(null);
      setPhase('register');
      await setLockdownState('Locked');
    } finally {
      factoryResetInFlightRef.current = false;
    }
  }, [realtimeRef]);

  const value: KioskContextValue = {
    phase,
    deviceId,
    deviceName,
    playerName,
    playerRole,
    activeSession,
    wsConnected,
    lastEvent,
    error,
    deviceStatus,
    maintenance,
    conflictDevice,
    online,
    loginNotice,
    clearLoginNotice,
    clearError,
    refresh,
    provisionDevice,
    adminAuthenticated: adminToken !== null,
    setupAuthenticated,
    clearSetupAuthenticated,
    enterSetup,
    exitSetup,
    adminLogin,
    playerLogin,
    playerLogout,
    startSession,
    reconcileSession: reconcileSessionOnce,
    endSession,
    dismissConflict,
    factoryReset,
    staffLockoutClearTick,
  };

  return <KioskContext.Provider value={value}>{children}</KioskContext.Provider>;
}

export function useKiosk(): KioskContextValue {
  const ctx = useContext(KioskContext);
  if (!ctx) throw new Error('useKiosk must be used within KioskProvider');
  return ctx;
}
