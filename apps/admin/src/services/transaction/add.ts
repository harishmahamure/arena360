import { http } from '@gaming-cafe/utils';
import type { PaymentMethodType } from '../../containers/transactions/schemas/transaction-schema';
import type { PaymentStatus, TransactionType } from './list';

export interface ProductPurchaseItem {
  productId: string;
  quantity: number;
}

export interface AddTransactionRequest {
  playerId: string;
  transactionType: TransactionType;
  productIds: ProductPurchaseItem[];
  paymentMethod: PaymentMethodType;
  paymentStatus?: PaymentStatus;
  notes?: string;
  transactionDate?: string;
  cashAmount: number;
  onlineAmount: number;
}

interface AddTransactionResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: AddTransactionResponseData;
}

interface AddTransactionResponseData {
  transaction: Transaction;
  remarks: string;
}

interface Transaction {
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
  notes: string;
  transactionDate: string;
}

export const addTransaction = async (transaction: AddTransactionRequest) => {
  return http.post<AddTransactionResponse>('/transactions', transaction);
};
