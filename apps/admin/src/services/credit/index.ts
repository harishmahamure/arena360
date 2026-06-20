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

export interface CreditSettlementListRow {
  id: string;
  playerId: string;
  playerUsername: string;
  shiftId: string;
  settledBy: string;
  settledByUsername: string;
  amount: number;
  paymentMethod: string;
  cashAmount?: number;
  onlineAmount?: number;
  notes?: string;
  settledAt: string;
  itemCount: number;
}

export interface CreditSettlementItemRow {
  transactionId: string;
  transactionType: string;
  transactionDate: string;
  originalAmount: number;
  amountApplied: number;
  remainingAfter: number;
}

export interface CreditSettlementDetail {
  id: string;
  playerId: string;
  playerUsername: string;
  shiftId: string;
  settledBy: string;
  settledByUsername: string;
  amount: number;
  paymentMethod: string;
  cashAmount?: number;
  onlineAmount?: number;
  notes?: string;
  settledAt: string;
  createdAt: string;
  updatedAt: string;
  items: CreditSettlementItemRow[];
}

interface PaginatedCreditSettlements {
  data: CreditSettlementListRow[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export const getCreditAccounts = async (filters: Record<string, unknown> = {}) =>
  http.get<PaginatedCreditAccounts>('/credit/accounts', {
    params: { limit: 20, page: 1, ...filters },
  });

export const getPlayerCredit = async (playerId: string) =>
  http.get<PlayerCreditDetail>(`/credit/players/${playerId}`);

export const settleCredit = async (payload: SettleCreditPayload) =>
  http.post<CreditSettlement>('/credit/settlements', payload);

export const getCreditSettlements = async (filters: Record<string, unknown> = {}) =>
  http.get<PaginatedCreditSettlements>('/credit/settlements', {
    params: { limit: 20, page: 1, sortBy: 'settledAt', sortOrder: 'DESC', ...filters },
  });

export const getCreditSettlementById = async (id: string) =>
  http.get<CreditSettlementDetail>(`/credit/settlements/${id}`);
