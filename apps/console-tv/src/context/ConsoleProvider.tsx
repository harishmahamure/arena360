import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useSessionReminders } from '../hooks/useSessionReminders';
import { ApiError, post, tokenCache } from '../lib/http';
import { collectFingerprint, setKeepScreenOn } from '../lib/native/ConsoleNative';
import { ConsoleRealtimeClient } from '../lib/realtime';
import {
  deviceIdFromToken,
  loadDeviceName,
  loadDeviceToken,
  persistDeviceName,
  persistDeviceToken,
} from '../lib/storage';

export type AppPhase = 'loading' | 'setup' | 'idle' | 'overlay';

export interface ActiveSession {
  id: string;
  startTime?: string;
  remainingMinutes: number | null;
}

export interface DeviceProvisionInput {
  name: string;
  deviceType: string;
  deviceSubType: string;
  location?: string;
}

interface ConsoleContextValue {
  phase: AppPhase;
  deviceId: string | null;
  deviceName: string | null;
  activeSession: ActiveSession | null;
  wsConnected: boolean;
  lastEvent: string | null;
  error: string | null;
  requestAdminOtp: (username: string, password: string) => Promise<string>;
  verifyRegistrationOtp: (otp: string, sessionOtpId: string) => Promise<void>;
  provisionDevice: (input: DeviceProvisionInput) => Promise<void>;
  adminAuthenticated: boolean;
}

const ConsoleContext = createContext<ConsoleContextValue | null>(null);

function toErrorMessage(e: unknown, fallback: string): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error && e.message) return e.message;
  return fallback;
}

export function ConsoleProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<AppPhase>('loading');
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [activeSession, setActiveSession] = useState<ActiveSession | null>(null);
  const [wsConnected, setWsConnected] = useState(false);
  const [lastEvent, setLastEvent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adminToken, setAdminToken] = useState<string | null>(null);

  const realtimeRef = useMemo(() => ({ current: new ConsoleRealtimeClient() }), []);

  useSessionReminders(activeSession?.remainingMinutes, phase === 'overlay');

  useEffect(() => {
    setKeepScreenOn(phase === 'overlay');
  }, [phase]);

  const connectWs = useCallback(
    (id: string) => {
      const rt = realtimeRef.current;
      rt.disconnect();
      rt.subscribe([`device:${id}`]);
      rt.onAny((frame) => {
        if (frame.event_type) setLastEvent(frame.event_type);

        if (frame.event_type === 'session.started') {
          const payload = frame.payload ?? {};
          const sessionId = String(payload.sessionId ?? '');
          const remaining =
            typeof payload.remainingMinutes === 'number' ? payload.remainingMinutes : null;
          const startTime = typeof payload.startTime === 'string' ? payload.startTime : undefined;
          setActiveSession({ id: sessionId, remainingMinutes: remaining, startTime });
          setPhase('overlay');
        }

        if (frame.event_type === 'session.ended') {
          setActiveSession(null);
          setPhase('idle');
        }

        if (frame.event_type === 'balance.updated') {
          const payload = frame.payload ?? {};
          if (typeof payload.remainingMinutes === 'number') {
            const minutes = payload.remainingMinutes as number;
            setActiveSession((prev) => (prev ? { ...prev, remainingMinutes: minutes } : prev));
          }
        }
      });
      rt.connect();
      const interval = setInterval(() => setWsConnected(rt.connected), 500);
      return () => clearInterval(interval);
    },
    [realtimeRef],
  );

  const bootstrap = useCallback(async () => {
    setError(null);
    const token = await loadDeviceToken();
    const name = await loadDeviceName();
    if (!token) {
      setPhase('setup');
      return;
    }
    tokenCache.device = token;
    setDeviceName(name);
    const id = deviceIdFromToken(token);
    setDeviceId(id);
    setPhase('idle');
    if (id) connectWs(id);
  }, [connectWs]);

  useEffect(() => {
    void bootstrap();
    return () => realtimeRef.current.disconnect();
  }, [bootstrap, realtimeRef]);

  const requestAdminOtp = useCallback(async (username: string, password: string) => {
    setError(null);
    try {
      const res = await post<{ transactionId: string }>('/auth/login/admin', {
        username,
        password,
      });
      return res.transactionId;
    } catch (e) {
      setError(toErrorMessage(e, 'Could not start admin login'));
      throw e;
    }
  }, []);

  const verifyRegistrationOtp = useCallback(async (otp: string, sessionOtpId: string) => {
    setError(null);
    try {
      const res = await post<{ accessToken: string }>('/auth/verify-otp', { otp, sessionOtpId });
      setAdminToken(res.accessToken);
    } catch (e) {
      setError(toErrorMessage(e, 'Invalid or expired OTP'));
      throw e;
    }
  }, []);

  const provisionDevice = useCallback(
    async (input: DeviceProvisionInput) => {
      setError(null);
      if (!adminToken) {
        setError('Administrator sign-in is required before registering this device.');
        throw new Error('admin token missing');
      }
      try {
        const fingerprint = await collectFingerprint();
        const result = await post<{
          accessToken: string;
          device: { id: string; name: string };
        }>(
          '/devices/provision',
          {
            fingerprint,
            name: input.name,
            deviceType: input.deviceType,
            deviceSubType: input.deviceSubType,
            location: input.location || undefined,
            serialNumber: fingerprint.serial,
          },
          adminToken,
        );
        await persistDeviceToken(result.accessToken);
        tokenCache.device = result.accessToken;
        setDeviceId(result.device.id);
        setDeviceName(result.device.name);
        await persistDeviceName(result.device.name);
        setAdminToken(null);
        setPhase('idle');
        connectWs(result.device.id);
      } catch (e) {
        setError(toErrorMessage(e, 'Device registration failed'));
        throw e;
      }
    },
    [adminToken, connectWs],
  );

  const value = useMemo<ConsoleContextValue>(
    () => ({
      phase,
      deviceId,
      deviceName,
      activeSession,
      wsConnected,
      lastEvent,
      error,
      requestAdminOtp,
      verifyRegistrationOtp,
      provisionDevice,
      adminAuthenticated: adminToken != null,
    }),
    [
      phase,
      deviceId,
      deviceName,
      activeSession,
      wsConnected,
      lastEvent,
      error,
      requestAdminOtp,
      verifyRegistrationOtp,
      provisionDevice,
      adminToken,
    ],
  );

  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}

export function useConsole(): ConsoleContextValue {
  const ctx = useContext(ConsoleContext);
  if (!ctx) throw new Error('useConsole must be used within ConsoleProvider');
  return ctx;
}
