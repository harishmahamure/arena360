import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';
import type { TransactionResponse } from './list';

export interface UpdateTransactionPayload {
  paymentStatus: PaymentStatusValue;
  notes?: string;
}

export const updateTransaction = async (id: string, payload: UpdateTransactionPayload) => {
  return http.patch<TransactionResponse>(`/transactions/${id}`, payload);
};
