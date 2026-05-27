import { http } from '@gaming-cafe/utils';
import type { ProductResponse } from './list';

export const getProductById = async (id: string) => {
  return http.get<ProductResponse>(`/products/${id}`);
};
