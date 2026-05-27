import { http } from '@gaming-cafe/utils';

export interface Shift {
  id: string;
  userId: string;
  clockIn: string;
  clockOut?: string | null;
  notes?: string | null;
  status: string;
  createdBy?: string | null;
  updatedBy?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ShiftListResponse {
  data: Shift[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export const clockIn = async (notes?: string) => http.post<Shift>('/shifts/clock-in', { notes });

export const clockOut = async (notes?: string) => http.patch<Shift>('/shifts/clock-out', { notes });

export const getActiveShift = async () => http.get<Shift | null>('/shifts/active');

export const getShift = async (id: string) => http.get<Shift>(`/shifts/${id}`);

export const getShifts = async (filters: Record<string, unknown> = {}) =>
  http.get<ShiftListResponse>('/shifts', {
    params: {
      ...filters,
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });
