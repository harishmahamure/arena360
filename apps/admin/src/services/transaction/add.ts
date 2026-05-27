import { http } from '@gaming-cafe/utils';
import type { PaymentMethodType } from '../../containers/transactions/schemas/transaction-schema';
import type { PaymentStatus, TransactionResponse, TransactionType } from './list';

export interface LineItemPayload {
  productId: string;
  quantity: number;
  unitPrice: number;
}

export interface CreateProductTransactionPayload {
  playerId: string;
  transactionType: TransactionType;
  amount: number;
  paymentMethod: PaymentMethodType;
  paymentStatus?: PaymentStatus;
  notes?: string;
  transactionDate?: string;
  cashAmount?: number;
  onlineAmount?: number;
  lineItems?: LineItemPayload[];
}

export const addTransaction = async (payload: CreateProductTransactionPayload) => {
  return http.post<TransactionResponse>('/transactions', payload);
};
