import { http } from '@gaming-cafe/utils';
import type { PaymentMethod, PaymentStatus, TransactionResponse, TransactionType } from './list';

export interface CreateTransactionPayload {
  playerId: string;
  transactionType: TransactionType;
  planId?: string;
  paymentMethod: PaymentMethod;
  paymentStatus?: PaymentStatus;
  notes?: string;
  transactionDate?: string;
}

export const addTransaction = async (payload: CreateTransactionPayload) => {
  return http.post<TransactionResponse>('/transactions', payload);
};
