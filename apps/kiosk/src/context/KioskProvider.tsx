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
import { formatPlayerLoginError } from '../lib/planErrors';
import { KioskRealtimeClient } from '../lib/realtime';
import { clearAllTokens, setLockdownState } from '../lib/tauriCommands';

export type AppPhase = 'loading' | 'register' | 'idle' | 'setup' | 'player-login' | 'session';

export interface ActiveSession {
  id: string;
  startTime: string;
  balanceId: string;
  remainingMinutes: number;
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
  refresh: () => Promise<void>;
  registerDevice: (code: string, name: string) => Promise<void>;
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
  factoryReset: () => Promise<void>;
}

const KioskContext = createContext<KioskContextValue | null>(null);

export function KioskProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<AppPhase>('loading');
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [playerName, setPlayerName] = useState<string | null>(null);
  const [activeSession, setActiveSession] = useState<ActiveSession | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const [lastEvent, setLastEvent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
          setActiveSession(null);
          setPhase('player-login');
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

  const registerDevice = useCallback(
    async (code: string, name: string) => {
      setError(null);
      const http = getHttpClient();
      const fingerprint = await import('../lib/tauriCommands').then((m) => m.collectFingerprint());
      const result = await http.post<{
        accessToken: string;
        device: { id: string; name: string };
      }>('/devices/register', {
        registrationCode: code,
        fingerprint,
        name,
      });
      await persistDeviceToken(result.accessToken);
      setDeviceId(result.device.id);
      setDeviceName(result.device.name);
      setPhase('idle');
      await setLockdownState('Locked');
      connectWs(result.device.id);
    },
    [connectWs],
  );

  const enterSetup = useCallback(async () => {
    setError(null);
    await setLockdownState('SetupRelaxed');
    setPhase('setup');
  }, []);

  const exitSetup = useCallback(async () => {
    await setLockdownState('Locked');
    setPhase('idle');
  }, []);

  const requestAdminOtp = useCallback(async (username: string, password: string) => {
    const http = getHttpClient();
    const res = await http.post<{ transactionId: string }>('/auth/login/admin', {
      username,
      password,
    });
    return res.transactionId;
  }, []);

  const adminLogin = useCallback(
    async (_username: string, _password: string, otp: string, sessionOtpId: string) => {
      setError(null);
      const http = getHttpClient();
      await http.post('/auth/verify-otp', { otp, sessionOtpId });
      await exitSetup();
    },
    [exitSetup],
  );

  const playerLogin = useCallback(
    async (username: string, password: string) => {
      setError(null);
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
    [connectWs, deviceId],
  );

  const playerLogout = useCallback(async () => {
    await clearPlayerSession();
    setPlayerName(null);
    setActiveSession(null);
    setPhase('idle');
    if (deviceId) connectWs(deviceId);
  }, [connectWs, deviceId]);

  const factoryReset = useCallback(async () => {
    await clearAllTokens();
    tokenCache.device = undefined;
    tokenCache.player = undefined;
    realtimeRef.current.disconnect();
    setDeviceId(null);
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
    refresh,
    registerDevice,
    enterSetup,
    exitSetup,
    adminLogin,
    requestAdminOtp,
    goToPlayerLogin: () => setPhase('player-login'),
    playerLogin,
    playerLogout,
    factoryReset,
  };

  return <KioskContext.Provider value={value}>{children}</KioskContext.Provider>;
}

export function useKiosk(): KioskContextValue {
  const ctx = useContext(KioskContext);
  if (!ctx) throw new Error('useKiosk must be used within KioskProvider');
  return ctx;
}
