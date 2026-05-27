import { http } from '@gaming-cafe/utils';
import type { PaymentStatus } from './list';

export interface UpdateTransactionStatusRequest {
  paymentStatus: PaymentStatus;
  notes?: string;
}

export const updateTransactionStatus = async (id: string, data: UpdateTransactionStatusRequest) => {
  return http.patch(`/transactions/${id}`, data);
};
