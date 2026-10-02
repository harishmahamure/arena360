import { http } from '@gaming-cafe/utils';

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

export interface KioskOrdersListResponse {
  data: KioskOrder[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface KioskOrderFilters {
  status?: string;
  deviceId?: string;
  page?: number;
  limit?: number;
}

export async function getKioskOrders(filters: KioskOrderFilters = {}) {
  const params = new URLSearchParams();
  if (filters.status) params.set('status', filters.status);
  if (filters.deviceId) params.set('deviceId', filters.deviceId);
  if (filters.page) params.set('page', String(filters.page));
  if (filters.limit) params.set('limit', String(filters.limit));
  const qs = params.toString();
  return http.get<KioskOrdersListResponse>(`/kiosk-orders${qs ? `?${qs}` : ''}`);
}

export async function getKioskOrder(id: string) {
  return http.get<KioskOrder>(`/kiosk-orders/${id}`);
}

export async function updateKioskOrderStatus(id: string, status: string) {
  return http.patch<KioskOrder>(`/kiosk-orders/${id}`, { status });
}
