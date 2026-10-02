import { http } from '@gaming-cafe/utils';
import type { TransactionResponse } from './list';

export const getTransactionById = async (id: string) => {
  return http.get<TransactionResponse>(`/transactions/${id}`);
};
