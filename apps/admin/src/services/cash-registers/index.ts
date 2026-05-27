import { http } from '@gaming-cafe/utils';

export interface CashRegister {
  id: string;
  shiftId: string;
  openedBy: string;
  closedBy?: string | null;
  openingBalance: number;
  openingDenominations?: Record<string, number> | null;
  closingBalance?: number | null;
  closingDenominations?: Record<string, number> | null;
  expectedClosing?: number | null;
  variance?: number | null;
  status: string;
  notes?: string | null;
  createdBy?: string | null;
  updatedBy?: string | null;
  createdAt: string;
  updatedAt: string;
  totalCashIn?: number | null;
  totalCashOut?: number | null;
  totalDeposited?: number | null;
}

export interface CashRegisterEntry {
  id: string;
  cashRegisterId: string;
  entryType: string;
  amount: number;
  reason?: string | null;
  referenceId?: string | null;
  referenceType?: string | null;
  createdBy?: string | null;
  createdAt: string;
}

export interface CashRegisterWithEntries extends CashRegister {
  entries: CashRegisterEntry[];
}

export interface CashRegisterListResponse {
  data: CashRegister[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface OpenCashRegisterDto {
  shiftId: string;
  openingBalance: number;
  openingDenominations?: Record<string, number>;
  notes?: string;
}

export interface CloseCashRegisterDto {
  closingBalance: number;
  closingDenominations?: Record<string, number>;
  notes?: string;
}

export interface CreateEntryDto {
  entryType: 'cash_in' | 'cash_out';
  amount: number;
  reason?: string;
  referenceId?: string;
  referenceType?: string;
}

export const openRegister = async (dto: OpenCashRegisterDto) =>
  http.post<CashRegister>('/cash-registers/open', dto);

export const closeRegister = async (id: string, dto: CloseCashRegisterDto) =>
  http.patch<CashRegister>(`/cash-registers/${id}/close`, dto);

export const addEntry = async (id: string, dto: CreateEntryDto) =>
  http.post<CashRegisterEntry>(`/cash-registers/${id}/entries`, dto);

export const getCashRegister = async (id: string) =>
  http.get<CashRegisterWithEntries>(`/cash-registers/${id}`);

export const getCashRegisters = async (filters: Record<string, unknown> = {}) =>
  http.get<CashRegisterListResponse>('/cash-registers', {
    params: {
      ...filters,
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });

export interface ReconcileCashRegisterDto {
  reconciliationNotes?: string;
}

export interface UpdateOpeningBalanceDto {
  openingBalance: number;
  openingDenominations?: Record<string, number>;
}

export const reconcileRegister = async (id: string, dto: ReconcileCashRegisterDto) =>
  http.patch<CashRegister>(`/cash-registers/${id}/reconcile`, dto);

export const updateOpeningBalance = async (id: string, dto: UpdateOpeningBalanceDto) =>
  http.patch<CashRegister>(`/cash-registers/${id}/update-opening`, dto);

export const DENOMINATIONS = {
  notes: [2000, 500, 200, 100, 50, 20, 10, 5, 2, 1],
  coins: [10, 5, 2, 1],
};
