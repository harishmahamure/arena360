import { http } from '@gaming-cafe/utils';

interface GetProductsResponse {
  data: ProductResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface ProductResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  name: string;
  description: string;
  price: string;
  dayPrice?: number;
  nightPrice?: number;
  purchasePricePerBox?: number | null;
  unitsPerPurchaseUnit?: number;
  unitId?: string | null;
  purchaseUnitId?: string | null;
  category: string;
  sku: null | string;
  stockQuantity: number;
  isActive: boolean;
}
export enum ProductCategory {
  BEVERAGE = 'beverage',
  SNACK = 'snack',
  MEAL = 'meal',
  OTHER = 'other',
}

export interface GetProductsFilters {
  name?: string;
  category?: ProductCategory;
  disabled?: 1 | 0;
  minPrice?: number;
  maxPrice?: number;
  locationId?: string;
  forSale?: boolean;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getProducts = async (filters: GetProductsFilters) => {
  return http.get<GetProductsResponse>('/products', {
    params: {
      ...filters,
      disabled: filters.disabled,
    },
  });
};
