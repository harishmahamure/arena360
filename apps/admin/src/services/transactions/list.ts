import {
  type PaymentMethodValue,
  PaymentStatus,
  type PaymentStatusValue,
  TransactionType,
  type TransactionTypeValue,
} from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export type { PaymentMethodValue as PaymentMethod };
export { PaymentStatus, TransactionType };

export interface TransactionResponse {
  id: string;
  playerId: string;
  transactionType: TransactionTypeValue;
  planId?: string;
  amount: number;
  paymentMethod: PaymentMethodValue;
  paymentStatus: PaymentStatusValue;
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
  transactionType?: TransactionTypeValue;
  planId?: string;
  paymentMethod?: PaymentMethodValue;
  paymentStatus?: PaymentStatusValue;
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
