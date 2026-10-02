import { http } from '@gaming-cafe/utils';

/** Quantity is a whole number in the ingredient's own unit (pieces, grams, millilitres). */
export interface RecipeIngredient {
  ingredientId: string;
  quantity: number;
}

export interface ProductOption {
  id?: string | null;
  name: string;
  priceDelta: number;
  /** Changes to the base recipe; a negative quantity removes an ingredient. */
  ingredients: RecipeIngredient[];
}

export interface ProductOptionGroup {
  id?: string | null;
  name: string;
  required: boolean;
  multiple: boolean;
  options: ProductOption[];
}

export interface ProductRecipe {
  items: RecipeIngredient[];
  optionGroups: ProductOptionGroup[];
}

export interface ProductCurrentPrice {
  productId: string;
  price: number;
  hasOptions: boolean;
  /** Present for made-to-order products: how many the ingredients can make. */
  madeToOrderAvailable?: number | null;
}

export const getProductRecipe = (productId: string) =>
  http.get<ProductRecipe>(`/products/${productId}/recipe`);

export const saveProductRecipe = (productId: string, recipe: ProductRecipe) =>
  http.put<ProductRecipe>(`/products/${productId}/recipe`, recipe);

export const getCurrentProductPrices = (locationId?: string) =>
  http.get<ProductCurrentPrice[]>('/products/current-prices', { params: { locationId } });
