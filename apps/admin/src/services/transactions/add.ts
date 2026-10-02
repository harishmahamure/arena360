import type {
  PaymentMethodValue,
  PaymentStatusValue,
  TransactionTypeValue,
} from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';
import type { TransactionResponse } from './list';

export interface CreateTransactionPayload {
  playerId: string;
  transactionType: TransactionTypeValue;
  planId?: string;
  amount?: number;
  paymentMethod: PaymentMethodValue;
  paymentStatus?: PaymentStatusValue;
  notes?: string;
  transactionDate?: string;
  cashAmount?: number;
  onlineAmount?: number;
  onlinePaymentRefLast4?: string;
}

export const addTransaction = async (payload: CreateTransactionPayload) => {
  return http.post<TransactionResponse>('/transactions', payload);
};
