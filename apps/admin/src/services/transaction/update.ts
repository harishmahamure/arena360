import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export interface UpdateTransactionStatusRequest {
  paymentStatus: PaymentStatusValue;
  notes?: string;
}

export const updateTransactionStatus = async (id: string, data: UpdateTransactionStatusRequest) => {
  return http.patch(`/transactions/${id}`, data);
};
