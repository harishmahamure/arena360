import { http } from '@gaming-cafe/utils';

export const deleteProduct = async (id: string) => {
  try {
    return http.delete(`/products/${id}`);
  } catch (_error) {
    throw new Error('Failed to deactivate product');
  }
};
