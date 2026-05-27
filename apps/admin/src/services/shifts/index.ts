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

export interface HandoverDepositInput {
  amount: number;
  denominations: Record<string, number>;
  notes?: string;
}

export interface ShiftHandoverInput {
  closingBalance: number;
  closingDenominations?: Record<string, number>;
  notes?: string;
  validatorUsername: string;
  validatorPassword: string;
  validatorTotp: string;
  deposit?: HandoverDepositInput;
}

export interface ShiftHandoverResponse {
  closedShift: Shift;
  cashRegister?: {
    id: string;
    variance?: number | null;
    expectedClosing?: number | null;
    closingBalance?: number | null;
  } | null;
  deposit?: { id: string; status: string; amount: number } | null;
  newAccessToken: string;
  newUser: {
    id: string;
    username: string;
    firstName?: string | null;
    lastName?: string | null;
    email?: string | null;
    role: string;
    isActive: boolean;
  };
  newShiftId: string;
}

export const handoverShift = async (input: ShiftHandoverInput) =>
  http.post<ShiftHandoverResponse>('/shifts/handover', input);

export const getExpectedClosing = async () =>
  http.get<{ registerId?: string | null; expectedClosing: number; openingBalance: number }>(
    '/cash-registers/active/expected-closing',
  );
