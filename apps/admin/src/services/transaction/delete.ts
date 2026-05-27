import { http } from '@gaming-cafe/utils';

export const deleteTransaction = async (id: string) => {
  try {
    return http.delete(`/transactions/${id}`);
  } catch (_error) {
    throw new Error('Failed to delete transaction');
  }
};
