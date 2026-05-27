import { http } from '@gaming-cafe/utils';
import type { PaymentStatus, TransactionResponse } from './list';

export interface UpdateTransactionPayload {
  paymentStatus: PaymentStatus;
  notes?: string;
}

export const updateTransaction = async (id: string, payload: UpdateTransactionPayload) => {
  return http.patch<TransactionResponse>(`/transactions/${id}`, payload);
};
