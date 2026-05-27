import { http } from '@gaming-cafe/utils';

export const deleteProduct = async (id: string) => {
  try {
    return http.patch(`/products/${id}/deactivate`);
  } catch (_error) {
    throw new Error('Failed to delete product');
  }
};
