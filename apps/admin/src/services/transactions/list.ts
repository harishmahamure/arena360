import { http } from '@gaming-cafe/utils';
import type { PaymentMethod } from '../transaction/list';

export type { PaymentMethod };

export enum TransactionType {
  PLAN_PURCHASE = 'plan_purchase',
  PRODUCT_PURCHASE = 'product_purchase',
}

export enum PaymentStatus {
  PENDING = 'pending',
  COMPLETED = 'completed',
  FAILED = 'failed',
  REFUNDED = 'refunded',
}

export interface TransactionResponse {
  id: string;
  playerId: string;
  transactionType: TransactionType;
  planId?: string;
  amount: number;
  paymentMethod: PaymentMethod;
  paymentStatus: PaymentStatus;
  notes?: string;
  transactionDate: string;
  createdAt: string;
  updatedAt: string;
  player?: {
    id: string;
    name: string;
    username: string;
  };
  plan?: {
    id: string;
    name: string;
    planType: string;
    price: number;
  };
  transactionProducts?: Array<{
    id: string;
    productId: string;
    quantity: number;
    priceAtPurchase: number;
    subtotal: number;
  }>;
}

interface GetTransactionsResponse {
  data: TransactionResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetTransactionsFilters {
  playerId?: string;
  transactionType?: TransactionType;
  planId?: string;
  paymentMethod?: PaymentMethod;
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

export const getTransactions = async (filters: GetTransactionsFilters = {}) => {
  return http.get<GetTransactionsResponse>('/transactions', {
    params: {
      ...filters,
      sortBy: filters.sortBy || 'transactionDate',
      sortOrder: filters.sortOrder || 'DESC',
      limit: filters.limit || 10,
      page: filters.page || 1,
    },
  });
};
