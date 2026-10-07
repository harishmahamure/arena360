import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { saveProductRecipe } from '../../services/product/recipe';
import ProductRecipeEditor from './ProductRecipeEditor';

vi.mock('../../hooks/useProductUnits', () => ({
  useProductUnits: () => ({
    units: [
      { id: 'g', type: 'gram', abbreviation: 'g' },
      { id: 'kg', type: 'kilogram', abbreviation: 'kg' },
    ],
  }),
}));
vi.mock('../../services/product/list', () => ({
  getProducts: vi.fn().mockResolvedValue({
    data: [
      {
        id: 'paneer',
        name: 'Paneer Topping',
        unitId: 'g',
        purchaseUnitId: 'kg',
        unitsPerPurchaseUnit: 1000,
        isRawMaterial: true,
      },
    ],
  }),
}));
vi.mock('../../services/product/recipe', () => ({
  getProductRecipe: vi
    .fn()
    .mockResolvedValue({ items: [{ ingredientId: 'paneer', quantity: 20 }], optionGroups: [] }),
  saveProductRecipe: vi.fn().mockImplementation(async (_id, recipe) => recipe),
}));
afterEach(cleanup);

it('labels kg-purchased paneer portions in grams and saves the 20 g recipe without converting it twice', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ProductRecipeEditor productId="pizza" isRawMaterial={false} canWrite />
    </QueryClientProvider>,
  );
  const quantity = await screen.findByRole('spinbutton', { name: 'Quantity (g)' });
  expect(quantity).toHaveValue(20);
  expect(screen.getByText('1 kg = 1,000 g')).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: 'Save recipe' }));
  await waitFor(() =>
    expect(saveProductRecipe).toHaveBeenCalledWith('pizza', {
      items: [{ ingredientId: 'paneer', quantity: 20 }],
      optionGroups: [],
    }),
  );
  client.clear();
});
