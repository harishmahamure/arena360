import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, expect, it, vi } from 'vitest';
import { addProduct } from '../../../services/product/add';
import AddNewProductPage from './ProductNewPage';

vi.mock('../../../services/product/add', () => ({ addProduct: vi.fn().mockResolvedValue({}) }));
vi.mock('../../../hooks/useProductUnits', () => {
  const units = [
    { id: '00000000-0000-4000-8000-000000000001', type: 'gram', name: 'Gram', abbreviation: 'g' },
    {
      id: '00000000-0000-4000-8000-000000000002',
      type: 'kilogram',
      name: 'Kilogram',
      abbreviation: 'kg',
    },
  ];
  return {
    useProductUnits: () => ({
      units,
      unitSelectOptions: units.map((unit) => ({ value: unit.id, label: unit.name })),
      defaultUnitIds: { sale: units[0]?.id, purchase: units[1]?.id },
      unitsReady: true,
      unitsMissing: false,
      unitsLoading: false,
    }),
  };
});

afterEach(cleanup);

it('generates an SKU from the product name while preserving manual edits and reset', async () => {
  render(
    <MemoryRouter>
      <AddNewProductPage />
    </MemoryRouter>,
  );

  const name = screen.getByRole('textbox', { name: /Product Name/ });
  const sku = screen.getByRole('textbox', { name: 'SKU' });
  expect(sku).toHaveValue('');
  expect(screen.getAllByText('SKU').filter((element) => element.tagName === 'LABEL')).toHaveLength(
    1,
  );

  fireEvent.change(name, { target: { value: 'Coca Cola 500ml' } });
  await waitFor(() => expect(sku).toHaveValue('COCA-COLA-500ML'));

  fireEvent.change(name, { target: { value: 'Coca Cola 1L' } });
  await waitFor(() => expect(sku).toHaveValue('COCA-COLA-1L'));

  fireEvent.mouseDown(screen.getByRole('combobox', { name: /Category/ }));
  fireEvent.click(await screen.findByRole('option', { name: 'Beverage' }));
  await waitFor(() => expect(sku).toHaveValue('COCA-COLA-1L-BEVERAGE'));

  fireEvent.change(sku, { target: { value: 'MY-CUSTOM-SKU' } });
  fireEvent.change(name, { target: { value: 'Coca Cola 2L' } });
  expect(sku).toHaveValue('MY-CUSTOM-SKU');

  fireEvent.click(screen.getByRole('button', { name: 'Reset Form' }));
  await waitFor(() => {
    expect(name).toHaveValue('');
    expect(sku).toHaveValue('');
  });

  fireEvent.change(name, { target: { value: 'Sprite' } });
  await waitFor(() => expect(sku).toHaveValue('SPRITE'));
});

it('stores kg input as grams and fixes the purchase conversion before review and submission', async () => {
  render(
    <MemoryRouter>
      <AddNewProductPage />
    </MemoryRouter>,
  );
  fireEvent.change(screen.getByRole('textbox', { name: /Product Name/ }), {
    target: { value: 'Paneer Topping' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  const conversion = await screen.findByRole('textbox', { name: 'Stock units per purchase unit' });
  await waitFor(() => expect(conversion).toHaveValue('1000'));
  expect(conversion).toHaveAttribute('readonly');
  expect(screen.getByText('1 kg = 1,000 g. Conversion is automatic.')).toBeVisible();

  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  const stock = await screen.findByRole('textbox', { name: 'Store stock (kg)' });
  fireEvent.focus(stock);
  fireEvent.change(stock, { target: { value: '0.02' } });
  fireEvent.blur(stock);
  expect(await screen.findByText('20 g in stock. Recipes use g.')).toBeVisible();
  fireEvent.focus(stock);
  fireEvent.change(stock, { target: { value: '20' } });
  fireEvent.blur(stock);
  expect(await screen.findByText('20,000 g in stock. Recipes use g.')).toBeVisible();

  fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Entry unit' }));
  fireEvent.click(await screen.findByRole('option', { name: 'g' }));
  expect(screen.getByRole('textbox', { name: 'Store stock (g)' })).toHaveValue('20000');
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Create Product' }));
  await waitFor(() =>
    expect(addProduct).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Paneer Topping',
        unitsPerPurchaseUnit: 1000,
        stockQuantity: 20_000,
      }),
    ),
  );
});
