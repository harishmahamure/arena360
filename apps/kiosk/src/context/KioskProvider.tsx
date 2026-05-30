import { ApiError } from '@gaming-cafe/utils';
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import {
  clearPlayerSession,
  getHttpClient,
  loadTokensIntoCache,
  persistDeviceToken,
  persistPlayerToken,
  tokenCache,
} from '../lib/http';
import { enqueueEndIntent, loadEndIntents, removeEndIntent } from '../lib/offlineQueue';
import { formatPlayerLoginError } from '../lib/planErrors';
import { KioskRealtimeClient } from '../lib/realtime';
import {
  clearAllTokens,
  clearTrackedProcesses,
  killTrackedProcesses,
  setLockdownState,
} from '../lib/tauriCommands';

export type AppPhase =
  | 'loading'
  | 'register'
  | 'idle'
  | 'setup'
  | 'player-login'
  | 'session'
  | 'already-in-session';

interface KioskSessionResponse {
  sessionId: string;
  balanceId: string;
  deviceId: string;
  startTime: string;
  remainingMinutes: number;
  resumed: boolean;
}

export interface ActiveSession {
  id: string;
  startTime: string;
  balanceId: string;
  remainingMinutes: number;
}

export interface DeviceRegistrationInput {
  code: string;
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
  /** Epoch ms when a force-end grace period elapses (admin force-end overlay). */
  forceEndGraceEndsAt: number | null;
  /** False when the backend is unreachable (failed polls); drives offline UX. */
  online: boolean;
  refresh: () => Promise<void>;
  registerDevice: (input: DeviceRegistrationInput) => Promise<void>;
  enterSetup: () => Promise<void>;
  exitSetup: () => Promise<void>;
  adminLogin: (
    username: string,
    password: string,
    otp: string,
    sessionOtpId: string,
  ) => Promise<void>;
  requestAdminOtp: (username: string, password: string) => Promise<string>;
  goToPlayerLogin: () => void;
  playerLogin: (username: string, password: string) => Promise<void>;
  playerLogout: () => Promise<void>;
  /** Start (or resume) the kiosk session for the signed-in player. */
  startSession: () => Promise<void>;
  /** End the current session with a reason and run process cleanup. */
  endSession: (reason?: string) => Promise<void>;
  /** Re-sync remaining minutes from the server; detects remote/auto end. */
  syncSession: () => Promise<void>;
  /** Dismiss the single-login conflict screen back to idle. */
  dismissConflict: () => void;
  factoryReset: () => Promise<void>;
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

export function KioskProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<AppPhase>('loading');
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [playerName, setPlayerName] = useState<string | null>(null);
  const [activeSession, setActiveSession] = useState<ActiveSession | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const [lastEvent, setLastEvent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deviceStatus, setDeviceStatus] = useState<string | null>(null);
  const [conflictDevice, setConflictDevice] = useState<string | null>(null);
  const [forceEndGraceEndsAt, setForceEndGraceEndsAt] = useState<number | null>(null);
  const [online, setOnline] = useState(true);

  const maintenance = deviceStatus === 'under_maintenance' || deviceStatus === 'out_of_service';

  const realtimeRef = useMemo(() => ({ current: new KioskRealtimeClient() }), []);

  const connectWs = useCallback(
    (id: string, playerId?: string) => {
      const rt = realtimeRef.current;
      rt.disconnect();
      rt.subscribe([`device:${id}`]);
      if (playerId) rt.subscribe([`user:${playerId}`]);
      rt.onAny((frame) => {
        if (frame.event_type) setLastEvent(frame.event_type);
        if (frame.event_type === 'session.ended') {
          const payload = frame.payload as { reason?: string } | undefined;
          if (payload?.reason === 'force') {
            // Admin force-end: keep the screen but show a grace overlay; the
            // SessionPage countdown triggers process cleanup at expiry (D14).
            setForceEndGraceEndsAt(Date.now() + 5 * 60 * 1000);
          } else {
            setActiveSession(null);
            setPlayerName(null);
            void clearPlayerSession();
            setPhase('idle');
          }
        }
        if (frame.event_type === 'balance.updated') {
          const payload = frame.payload as { remainingMinutes?: number } | undefined;
          if (typeof payload?.remainingMinutes === 'number') {
            const minutes = payload.remainingMinutes;
            setActiveSession((prev) => (prev ? { ...prev, remainingMinutes: minutes } : prev));
          }
        }
        if (frame.event_type === 'device.status_changed') {
          const payload = frame.payload as { status?: string } | undefined;
          if (typeof payload?.status === 'string') {
            setDeviceStatus(payload.status);
            // If the station goes into maintenance while idle/login, kick back to idle.
            if (payload.status === 'under_maintenance' || payload.status === 'out_of_service') {
              setPhase((prev) => (prev === 'player-login' ? 'idle' : prev));
            }
          }
        }
      });
      rt.connect();
      const interval = setInterval(() => setWsConnected(rt.connected), 500);
      return () => clearInterval(interval);
    },
    [realtimeRef],
  );

  const refresh = useCallback(async () => {
    setError(null);
    await loadTokensIntoCache();
    if (!tokenCache.device) {
      setPhase('register');
      await setLockdownState('Locked');
      return;
    }
    setPhase('idle');
    await setLockdownState('Locked');
    setDeviceName(readStoredDeviceName());
    // Device id from JWT payload (base64 middle segment)
    try {
      const payload = JSON.parse(atob(tokenCache.device.split('.')[1] ?? ''));
      const id = payload.userId as string;
      setDeviceId(id);
      connectWs(id);
    } catch {
      setDeviceId(null);
    }
  }, [connectWs]);

  useEffect(() => {
    void refresh();
    return () => realtimeRef.current.disconnect();
  }, [refresh, realtimeRef]);

  // Reconcile the React phase with the native lockdown state. If the OS layer
  // re-locks (e.g. setup idle timeout fires) while we're still on the setup
  // screen, drop back to idle.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/event').then(({ listen }) =>
      listen<string>('lockdown-changed', (event) => {
        if (event.payload === 'Locked') {
          setPhase((prev) => (prev === 'setup' ? 'idle' : prev));
        }
      }).then((fn) => {
        unlisten = fn;
      }),
    );
    return () => unlisten?.();
  }, []);

  const registerDevice = useCallback(
    async (input: DeviceRegistrationInput) => {
      setError(null);
      try {
        const http = getHttpClient();
        const fingerprint = await import('../lib/tauriCommands').then((m) =>
          m.collectFingerprint(),
        );
        const result = await http.post<{
          accessToken: string;
          device: { id: string; name: string };
        }>('/devices/register', {
          registrationCode: input.code,
          fingerprint,
          name: input.name,
          deviceType: input.deviceType,
          deviceSubType: input.deviceSubType,
          location: input.location || undefined,
          serialNumber: input.serialNumber || fingerprint.serial,
        });
        await persistDeviceToken(result.accessToken);
        setDeviceId(result.device.id);
        setDeviceName(result.device.name);
        storeDeviceName(result.device.name);
        setPhase('idle');
        await setLockdownState('Locked');
        connectWs(result.device.id);
      } catch (e) {
        setError(toErrorMessage(e, 'Device registration failed'));
        throw e;
      }
    },
    [connectWs],
  );

  const enterSetup = useCallback(async () => {
    setError(null);
    // Stay Locked until an admin authenticates (ADR-0020): the setup gesture
    // only reveals the login form, it does not relax lockdown.
    setPhase('setup');
  }, []);

  const exitSetup = useCallback(async () => {
    await setLockdownState('Locked');
    setPhase('idle');
  }, []);

  const requestAdminOtp = useCallback(async (username: string, password: string) => {
    setError(null);
    try {
      const http = getHttpClient();
      const res = await http.post<{ transactionId: string }>('/auth/login/admin', {
        username,
        password,
      });
      return res.transactionId;
    } catch (e) {
      setError(toErrorMessage(e, 'Could not start admin login'));
      throw e;
    }
  }, []);

  const adminLogin = useCallback(
    async (_username: string, _password: string, otp: string, sessionOtpId: string) => {
      setError(null);
      try {
        const http = getHttpClient();
        await http.post('/auth/verify-otp', { otp, sessionOtpId });
        // Admin authenticated: now relax lockdown for setup work (ADR-0020).
        await setLockdownState('SetupRelaxed');
      } catch (e) {
        setError(toErrorMessage(e, 'Invalid or expired OTP'));
        throw e;
      }
    },
    [],
  );

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
          user: { id: string; username: string };
          activeSession?: ActiveSession | null;
        }>('/auth/login/player', { username, password });

        await persistPlayerToken(res.accessToken);
        setPlayerName(res.user.username);
        if (res.activeSession) {
          setActiveSession(res.activeSession);
        }
        setPhase('session');
        await setLockdownState('Locked');
        if (deviceId) connectWs(deviceId, res.user.id);
      } catch (e) {
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
    setActiveSession(null);
    setPhase('idle');
    if (deviceId) connectWs(deviceId);
  }, [connectWs, deviceId]);

  const startSession = useCallback(async () => {
    setError(null);
    const http = getHttpClient();
    try {
      const res = await http.post<KioskSessionResponse>('/kiosk/sessions', {});
      setActiveSession({
        id: res.sessionId,
        startTime: res.startTime,
        balanceId: res.balanceId,
        remainingMinutes: res.remainingMinutes,
      });
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

  const endSession = useCallback(
    async (reason = 'voluntary') => {
      const sessionId = activeSession?.id;
      if (sessionId) {
        try {
          await getHttpClient().patch(`/kiosk/sessions/${sessionId}/end`, { reason });
        } catch {
          // Offline at end time: queue an idempotent replay for reconnect (D18).
          enqueueEndIntent(sessionId, reason === 'voluntary' ? 'offline_reconcile' : reason);
        }
      }
      try {
        await killTrackedProcesses();
        await clearTrackedProcesses();
      } catch {
        // Process cleanup is best-effort off-Windows / when nothing tracked.
      }
      setActiveSession(null);
      setForceEndGraceEndsAt(null);
      await clearPlayerSession();
      setPlayerName(null);
      setPhase('idle');
      if (deviceId) connectWs(deviceId);
    },
    [activeSession, connectWs, deviceId],
  );

  const syncSession = useCallback(async () => {
    try {
      const res = await getHttpClient().get<KioskSessionResponse | null>('/kiosk/sessions/current');
      setOnline(true);
      void flushEndIntents();
      if (!res) {
        // Session ended elsewhere (auto-expiry or admin end) — return to idle.
        setActiveSession(null);
        setPlayerName(null);
        await clearPlayerSession();
        setPhase('idle');
        if (deviceId) connectWs(deviceId);
        return;
      }
      setActiveSession((prev) =>
        prev
          ? { ...prev, remainingMinutes: res.remainingMinutes }
          : {
              id: res.sessionId,
              startTime: res.startTime,
              balanceId: res.balanceId,
              remainingMinutes: res.remainingMinutes,
            },
      );
    } catch {
      // Offline: leave the last-known countdown running (offline grace, K6).
      setOnline(false);
    }
  }, [connectWs, deviceId, flushEndIntents]);

  const dismissConflict = useCallback(() => {
    setConflictDevice(null);
    setPlayerName(null);
    void clearPlayerSession();
    setPhase('idle');
    if (deviceId) connectWs(deviceId);
  }, [connectWs, deviceId]);

  // Replay any queued end intents whenever connectivity is (re)established.
  useEffect(() => {
    if (online) void flushEndIntents();
  }, [online, flushEndIntents]);

  const factoryReset = useCallback(async () => {
    await clearAllTokens();
    tokenCache.device = undefined;
    tokenCache.player = undefined;
    realtimeRef.current.disconnect();
    setDeviceId(null);
    setDeviceName(null);
    storeDeviceName(null);
    setPhase('register');
    await setLockdownState('Locked');
  }, [realtimeRef]);

  const value: KioskContextValue = {
    phase,
    deviceId,
    deviceName,
    playerName,
    activeSession,
    wsConnected,
    lastEvent,
    error,
    deviceStatus,
    maintenance,
    conflictDevice,
    forceEndGraceEndsAt,
    online,
    refresh,
    registerDevice,
    enterSetup,
    exitSetup,
    adminLogin,
    requestAdminOtp,
    goToPlayerLogin: () => setPhase('player-login'),
    playerLogin,
    playerLogout,
    startSession,
    endSession,
    syncSession,
    dismissConflict,
    factoryReset,
  };

  return <KioskContext.Provider value={value}>{children}</KioskContext.Provider>;
}

export function useKiosk(): KioskContextValue {
  const ctx = useContext(KioskContext);
  if (!ctx) throw new Error('useKiosk must be used within KioskProvider');
  return ctx;
}
