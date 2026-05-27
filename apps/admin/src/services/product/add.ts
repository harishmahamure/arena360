import { http } from '@gaming-cafe/utils';
import type { ProductCategory } from './list';

interface AddProductRequest {
  name: string;
  description: string;
  price: number;
  category: ProductCategory;
  sku: string;
  stockQuantity: number;
  isActive: boolean;
}

export const addProduct = async (product: AddProductRequest) => {
  return http.post('/products', product);
};
