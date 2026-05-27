import { http } from '@gaming-cafe/utils';

export const deleteTransaction = async (id: string) => {
  return http.delete(`/transactions/${id}`);
};
