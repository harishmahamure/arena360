import { http } from '@gaming-cafe/utils';

export interface CreditSummary {
  playerId: string;
  creditLimit: number;
  outstanding: number;
  available: number;
  creditEnabled: boolean;
}

export interface OutstandingTxn {
  transactionId: string;
  transactionType: string;
  amount: number;
  paidAmount: number;
  remaining: number;
  paymentStatus: string;
  transactionDate: string;
  notes?: string;
}

export interface PlayerCreditDetail {
  summary: CreditSummary;
  transactions: OutstandingTxn[];
}

export type CreditPlayerRow = {
  playerId: string;
  username: string;
  firstName?: string;
  lastName?: string;
  phoneNumber?: string;
  creditLimit: number;
  outstanding: number;
  available: number;
};

interface PaginatedCreditAccounts {
  data: CreditPlayerRow[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface SettleItem {
  transactionId: string;
  amount: number;
}

export interface SettleCreditPayload {
  playerId: string;
  items: SettleItem[];
  paymentMethod: string;
  cashAmount?: number;
  onlineAmount?: number;
  notes?: string;
}

export interface CreditSettlement {
  id: string;
  playerId: string;
  shiftId: string;
  settledBy: string;
  amount: number;
  paymentMethod: string;
  cashAmount?: number;
  onlineAmount?: number;
  notes?: string;
  settledAt: string;
}

export const getCreditAccounts = async (filters: Record<string, unknown> = {}) =>
  http.get<PaginatedCreditAccounts>('/credit/accounts', {
    params: { limit: 20, page: 1, ...filters },
  });

export const getPlayerCredit = async (playerId: string) =>
  http.get<PlayerCreditDetail>(`/credit/players/${playerId}`);

export const settleCredit = async (payload: SettleCreditPayload) =>
  http.post<CreditSettlement>('/credit/settlements', payload);
