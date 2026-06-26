import type { PaymentStatusValue, TransactionTypeValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';
import type { PaymentMethodType } from '../../containers/transactions/schemas/transaction-schema';
import type { TransactionResponse } from './list';

export interface LineItemPayload {
  productId: string;
  quantity: number;
  unitPrice: number;
}

export interface CreateProductTransactionPayload {
  playerId: string;
  transactionType: TransactionTypeValue;
  amount: number;
  paymentMethod: PaymentMethodType;
  paymentStatus?: PaymentStatusValue;
  notes?: string;
  transactionDate?: string;
  cashAmount?: number;
  onlineAmount?: number;
  lineItems?: LineItemPayload[];
  saleLocationId?: string;
  kioskOrderId?: string;
}

export const addTransaction = async (payload: CreateProductTransactionPayload) => {
  return http.post<TransactionResponse>('/transactions', payload);
};
