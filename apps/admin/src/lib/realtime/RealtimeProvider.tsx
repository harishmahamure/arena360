import { isImportantNotificationKind, Permission } from '@gaming-cafe/contracts';
import { local, toastUtils } from '@gaming-cafe/utils';
import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext, useEffect, useState } from 'react';
import { useSelector } from '../../hooks/store';
import { sessionPermissions } from '../authSession';
import { RealtimeClient, type ServerFrame } from './client';

interface RealtimeContextValue {
  client: RealtimeClient | null;
  status: 'connecting' | 'connected' | 'offline';
}

const RealtimeContext = createContext<RealtimeContextValue>({ client: null, status: 'offline' });

export function getWsUrl(): string {
  if (import.meta.env.VITE_GATEWAY_URL) return import.meta.env.VITE_GATEWAY_URL;
  const url = new URL(import.meta.env.VITE_API_URL || 'http://localhost:3000');
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = `${url.pathname.replace(/\/$/, '')}/realtime`;
  url.search = '';
  url.hash = '';
  return url.toString();
}

export function RealtimeProvider({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const { id: userId, role } = useSelector((state) => state.auth);
  const accessToken = local.get<string>('accessToken');
  const [contextValue, setContextValue] = useState<RealtimeContextValue>({
    client: null,
    status: 'offline',
  });

  useEffect(() => {
    if (!accessToken || !userId) {
      setContextValue({ client: null, status: 'offline' });
      return;
    }
    let active = true;
    const client = new RealtimeClient(getWsUrl(), (status) => {
      if (!active) return;
      setContextValue({ client, status });
      if (status === 'connected') void queryClient.invalidateQueries();
    });
    // Events may affect aggregates and detail queries under different keys.
    // Coalesce a burst into one active-query refresh; inactive queries become stale.
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;
    const unsubRefresh = client.onAny((frame) => {
      if (frame.type !== 'Event' || refreshTimer) return;
      refreshTimer = setTimeout(() => {
        refreshTimer = undefined;
        void queryClient.invalidateQueries();
      }, 250);
    });

    const channels: string[] = ['public'];
    if (role === 'admin') {
      channels.push('admin', 'staff');
    } else if (role === 'staff') {
      channels.push('staff');
    }
    const permissions = sessionPermissions();
    if (permissions.includes(Permission.SettingsRead) || permissions.includes(Permission.RulesRead))
      channels.push('configuration');
    if (userId) {
      channels.push(`user:${userId}`);
    }

    client.subscribe(channels);

    const unsubApprovalReq = client.on('approval.requested', (frame: ServerFrame) => {
      const entity = (frame.payload?.entity_type as string) ?? 'item';
      toastUtils.info(`New ${entity} awaiting approval`);
    });

    const unsubApprovalDec = client.on('approval.decided', (frame: ServerFrame) => {
      const status = (frame.payload?.status as string) ?? 'decided';
      const entity = (frame.payload?.entity_type as string) ?? 'item';
      toastUtils.info(`Your ${entity} was ${status}`);
    });

    const unsubNotification = client.on('notification.created', (frame: ServerFrame) => {
      const title = frame.payload?.title as string | undefined;
      const kind = frame.payload?.kind as string | undefined;
      if (title && kind && isImportantNotificationKind(kind)) {
        toastUtils.info(title);
      }
    });

    const unsubKioskOrder = client.on('kiosk_order.placed', (frame: ServerFrame) => {
      const deviceName = (frame.payload?.deviceName as string) ?? 'Station';
      const username = (frame.payload?.playerUsername as string) ?? 'player';
      toastUtils.info(`New order from ${deviceName} — ${username}`);
    });

    client.connect();
    return () => {
      active = false;
      clearTimeout(refreshTimer);
      unsubRefresh();
      unsubApprovalReq();
      unsubApprovalDec();
      unsubNotification();
      unsubKioskOrder();
      client.disconnect();
    };
  }, [queryClient, userId, role, accessToken]);

  useEffect(() => {
    if (!accessToken || contextValue.status === 'connected') return;
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') void queryClient.invalidateQueries();
    }, 30_000);
    return () => clearInterval(timer);
  }, [accessToken, contextValue.status, queryClient]);

  return <RealtimeContext value={contextValue}>{children}</RealtimeContext>;
}

export function useRealtime(): RealtimeClient | null {
  return useContext(RealtimeContext).client;
}

export function useRealtimeStatus() {
  return useContext(RealtimeContext).status;
}
