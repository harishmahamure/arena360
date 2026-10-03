import { http } from '@gaming-cafe/utils';
import type { CreateProductFormData } from '../../containers/products/schemas/product-schema';

export const updateProduct = async (
  id: string,
  product: CreateProductFormData & { nightPrice?: number; dayPrice?: number },
) => {
  return http.patch(`/products/${id}`, product);
};
