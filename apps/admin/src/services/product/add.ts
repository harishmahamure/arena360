import { http } from '@gaming-cafe/utils';
import type { ProductCategory } from './list';

interface AddProductRequest {
  name: string;
  description: string;
  price: number;
  dayPrice?: number;
  nightPrice: number;
  purchasePricePerBox?: number;
  unitsPerPurchaseUnit?: number;
  unitId?: string;
  purchaseUnitId?: string;
  category: ProductCategory;
  sku: string;
  stockQuantity: number;
  isActive: boolean;
}

export const addProduct = async (product: AddProductRequest) => {
  return http.post('/products', {
    ...product,
    dayPrice: product.dayPrice ?? product.price,
  });
};
