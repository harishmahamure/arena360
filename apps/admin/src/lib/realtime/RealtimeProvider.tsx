import { local, toastUtils } from '@gaming-cafe/utils';
import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext, useEffect, useMemo, useRef } from 'react';
import { useSelector } from '../../hooks/store';
import { RealtimeClient, type ServerFrame } from './client';

interface RealtimeContextValue {
  client: RealtimeClient | null;
}

const RealtimeContext = createContext<RealtimeContextValue>({ client: null });

function getWsUrl(): string {
  const apiUrl =
    typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL
      ? import.meta.env.VITE_API_URL
      : 'http://localhost:3000';

  const wsProtocol = apiUrl.startsWith('https') ? 'wss' : 'ws';
  const host = apiUrl.replace(/^https?:\/\//, '');
  return `${wsProtocol}://${host}/realtime`;
}

export function RealtimeProvider({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const { id: userId, role } = useSelector((state) => state.auth);
  const accessToken = local.get<string>('accessToken');
  const clientRef = useRef<RealtimeClient | null>(null);

  const contextValue = useMemo<RealtimeContextValue>(() => {
    const client = new RealtimeClient(getWsUrl());
    clientRef.current = client;
    return { client };
  }, []);

  useEffect(() => {
    const client = clientRef.current;
    if (!client || !accessToken) return;

    client.connect();

    const channels: string[] = ['public'];
    if (role === 'admin') {
      channels.push('admin', 'staff');
    } else if (role === 'staff') {
      channels.push('staff');
    }
    if (userId) {
      channels.push(`user:${userId}`);
    }

    client.subscribe(channels);

    const unsubSale = client.on('transaction.sale_completed', (frame: ServerFrame) => {
      toastUtils.success(`Sale completed: ₹${frame.payload?.amount ?? ''}`);
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
    });

    const unsubApprovalReq = client.on('approval.requested', (frame: ServerFrame) => {
      const entity = (frame.payload?.entity_type as string) ?? 'item';
      toastUtils.info(`New ${entity} awaiting approval`);
      queryClient.invalidateQueries({ queryKey: ['cash-deposits'] });
      queryClient.invalidateQueries({ queryKey: ['expenses'] });
    });

    const unsubApprovalDec = client.on('approval.decided', (frame: ServerFrame) => {
      const status = (frame.payload?.status as string) ?? 'decided';
      const entity = (frame.payload?.entity_type as string) ?? 'item';
      toastUtils.info(`Your ${entity} was ${status}`);
      queryClient.invalidateQueries({ queryKey: ['cash-deposits'] });
      queryClient.invalidateQueries({ queryKey: ['expenses'] });
    });

    const unsubSession = client.on('session.started', () => {
      queryClient.invalidateQueries({ queryKey: ['sessions'] });
      queryClient.invalidateQueries({ queryKey: ['devices'] });
    });

    const unsubSessionEnd = client.on('session.ended', () => {
      queryClient.invalidateQueries({ queryKey: ['sessions'] });
      queryClient.invalidateQueries({ queryKey: ['devices'] });
    });

    const unsubDevice = client.on('device.status_changed', () => {
      queryClient.invalidateQueries({ queryKey: ['devices'] });
    });

    return () => {
      unsubSale();
      unsubApprovalReq();
      unsubApprovalDec();
      unsubSession();
      unsubSessionEnd();
      unsubDevice();
      client.disconnect();
    };
  }, [queryClient, userId, role, accessToken]);

  return <RealtimeContext value={contextValue}>{children}</RealtimeContext>;
}

export function useRealtime(): RealtimeClient | null {
  return useContext(RealtimeContext).client;
}
