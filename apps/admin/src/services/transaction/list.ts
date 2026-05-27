import { http } from '@gaming-cafe/utils';
import type { PaymentMethodType } from '../../containers/transactions/schemas/transaction-schema';

export enum PaymentStatus {
  PENDING = 'pending',
  COMPLETED = 'completed',
  FAILED = 'failed',
  REFUNDED = 'refunded',
  CANCELLED = 'cancelled',
}

export enum TransactionType {
  PRODUCT_PURCHASE = 'product_purchase',
}

interface TransactionListResponseData {
  data: Transaction[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface Transaction {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
  playerId: string;
  transactionType: string;
  planId?: string | null;
  amount: number;
  cashAmount?: number | null;
  onlineAmount?: number | null;
  paymentMethod: string;
  paymentStatus: string;
  notes?: string | null;
  transactionDate: string;
  transactionProducts?: TransactionProduct[];
  player?: Player;
  plan?: null;
}

export type TransactionResponse = Transaction;
export type PaymentMethod = PaymentMethodType;

interface Player {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  username: string;
  phoneNumber?: string | null;
  isActive: boolean;
  firstName: string;
  lastName: string;
  role: string;
  sessionOtpId: null;
  sessionOtp: string;
}

export interface TransactionProduct {
  id: string;
  transactionId: string;
  productId: string;
  quantity: number;
  unitPrice: number;
  productName: string;
  productSku?: string | null;
  productPrice: number;
  createdAt: string;
}

export interface TransactionWithLineItems extends Transaction {
  lineItems: TransactionProduct[];
}

export interface GetTransactionsFilters {
  playerId?: string;
  transactionType?: TransactionType;
  paymentMethod?: PaymentMethodType;
  paymentStatus?: PaymentStatus;
  transactionDateFrom?: string;
  transactionDateTo?: string;
  minAmount?: number;
  maxAmount?: number;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getTransactions = async (filters: GetTransactionsFilters) => {
  return http.get<TransactionListResponseData>('/transactions', {
    params: filters,
  });
};

export const getTransactionById = async (id: string) => {
  return http.get<TransactionWithLineItems>(`/transactions/${id}`);
};
