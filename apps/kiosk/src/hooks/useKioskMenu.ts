import { useCallback, useEffect, useState } from 'react';
import { getHttpClient } from '../lib/http';

export interface KioskMenuProduct {
  id: string;
  name: string;
  description?: string | null;
  category: string;
  price: number;
  stockAvailable: number;
  inStock: boolean;
}

export interface KioskOrderLineItem {
  id: string;
  productId: string;
  quantity: number;
  productName: string;
  unitPrice: number;
}

export interface KioskOrder {
  id: string;
  sessionId: string;
  playerId: string;
  deviceId: string;
  status: string;
  playerNote?: string | null;
  transactionId?: string | null;
  createdAt: string;
  updatedAt: string;
  fulfilledAt?: string | null;
  lineItems: KioskOrderLineItem[];
  deviceName?: string | null;
  playerUsername?: string | null;
}

export function useKioskProducts() {
  const [products, setProducts] = useState<KioskMenuProduct[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const http = getHttpClient();
      const res = await http.get<KioskMenuProduct[]>('/kiosk/products');
      setProducts(res ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load menu');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { products, loading, error, refresh };
}

export function useKioskOrder() {
  const [currentOrder, setCurrentOrder] = useState<KioskOrder | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const http = getHttpClient();
      const res = await http.get<KioskOrder | null>('/kiosk/orders/current');
      setCurrentOrder(res ?? null);
    } catch {
      setCurrentOrder(null);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => void refresh(), 15000);
    return () => window.clearInterval(id);
  }, [refresh]);

  const submitOrder = useCallback(
    async (lineItems: { productId: string; quantity: number }[], note?: string) => {
      setSubmitting(true);
      try {
        const http = getHttpClient();
        const res = await http.post<KioskOrder>('/kiosk/orders', { lineItems, note });
        setCurrentOrder(res ?? null);
        return res;
      } finally {
        setSubmitting(false);
      }
    },
    [],
  );

  return { currentOrder, submitting, submitOrder, refresh };
}
