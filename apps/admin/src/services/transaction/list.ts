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
  onlinePaymentRefLast4?: string | null;
  paymentMethod: string;
  paymentStatus: string;
  notes?: string | null;
  transactionDate: string;
  transactionProducts?: TransactionProduct[];
  player?: {
    id: string;
    username: string;
    name?: string;
  };
  plan?: {
    id: string;
    name: string;
    planType: string;
    price: number;
  } | null;
}

export type TransactionResponse = Transaction;

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
  transactionType?: TransactionTypeValue;
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

export const getTransactions = async (filters: GetTransactionsFilters) => {
  return http.get<TransactionListResponseData>('/transactions', {
    params: filters,
  });
};

export const getTransactionById = async (id: string) => {
  return http.get<TransactionWithLineItems>(`/transactions/${id}`);
};
