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
  deletedAt: null;
  playerId: string;
  transactionType: string;
  planId: null;
  amount: string;
  paymentMethod: string;
  paymentStatus: string;
  notes: null | string;
  transactionDate: string;
  transactionProducts: TransactionProduct[];
  player: Player;
  plan: null;
}

export type TransactionResponse = Transaction;
export type PaymentMethod = PaymentMethodType;

interface Player {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  email: null | string;
  username: string;
  isActive: boolean;
  firstName: string;
  lastName: string;
  role: string;
  sessionOtpId: null;
  sessionOtp: string;
}

interface TransactionProduct {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  transactionId: string;
  productId: string;
  quantity: number;
  priceAtPurchase: string;
  subtotal: string;
  notes: null;
  product: Product;
}

interface Product {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  name: string;
  description: string;
  price: string;
  category: string;
  sku: string;
  stockQuantity: number;
  isActive: boolean;
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
