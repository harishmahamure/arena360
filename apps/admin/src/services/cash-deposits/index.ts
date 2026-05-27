import { http } from '@gaming-cafe/utils';

export interface CashDeposit {
  id: string;
  cashRegisterId: string;
  shiftId: string;
  initiatedBy: string;
  approvedBy?: string | null;
  amount: number;
  denominations: Record<string, number>;
  depositType?: string | null;
  status: string;
  approvedAt?: string | null;
  rejectionReason?: string | null;
  notes?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CashDepositListResponse {
  data: CashDeposit[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface InitiateDepositDto {
  cashRegisterId: string;
  shiftId: string;
  amount: number;
  denominations: Record<string, number>;
  notes?: string;
}

export const getCashDeposits = async (filters: Record<string, unknown> = {}) =>
  http.get<CashDepositListResponse>('/cash-deposits', {
    params: {
      ...filters,
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });

export const getCashDeposit = async (id: string) => http.get<CashDeposit>(`/cash-deposits/${id}`);

export const initiateDeposit = async (dto: InitiateDepositDto) =>
  http.post<CashDeposit>('/cash-deposits', dto);

export const approveDeposit = async (id: string, depositType: 'bank' | 'home') =>
  http.patch<CashDeposit>(`/cash-deposits/${id}/approve`, { depositType });

export const rejectDeposit = async (id: string, rejectionReason: string) =>
  http.patch<CashDeposit>(`/cash-deposits/${id}/reject`, { rejectionReason });
