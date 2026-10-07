import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { PosPlayerPickerProps } from '../../../containers/sales/PosPlayerPicker';
import { getVenueLocations } from '../../../services/config';
import { getDeviceById } from '../../../services/devices/getById';
import { getInventoryLocations, getLocationStock } from '../../../services/inventory';
import { getKioskOrder } from '../../../services/kiosk-orders';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { getCurrentProductPrices } from '../../../services/product/recipe';
import { addTransaction } from '../../../services/transaction/add';
import CreateProductTransactionPage from './ProductTransactionNewPage';

vi.mock('../../../components/ActiveShiftGuard', () => ({
  ActiveShiftGuard: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('../../../containers/sales/PosPlayerPicker', () => ({
  PosPlayerPicker: ({ onChange }: PosPlayerPickerProps) => (
    <button type="button" onClick={() => onChange({ id: 'player', username: 'Demo player' })}>
      Select demo player
    </button>
  ),
}));
vi.mock('../../../services/inventory', () => ({
  getInventoryLocations: vi.fn(),
  getLocationStock: vi.fn(),
}));
vi.mock('../../../services/config', () => ({ getVenueLocations: vi.fn() }));
vi.mock('../../../services/devices/getById', () => ({ getDeviceById: vi.fn() }));
vi.mock('../../../services/product/list', () => ({ getProducts: vi.fn() }));
vi.mock('../../../services/product/recipe', () => ({
  getCurrentProductPrices: vi.fn(),
  getProductRecipe: vi.fn(),
}));
vi.mock('../../../services/kiosk-orders', () => ({ getKioskOrder: vi.fn() }));
vi.mock('../../../services/transaction/add', () => ({ addTransaction: vi.fn() }));

const product = (id: string, name: string): ProductResponse => ({
  id,
  name,
  description: '',
  price: '50',
  stockQuantity: 99,
  isActive: true,
  createdAt: '',
  updatedAt: '',
  deletedAt: null,
  category: 'snack',
  sku: null,
});
const page = <T,>(data: T[], pageNumber = 1, totalPages = 1) => ({
  data,
  page: pageNumber,
  totalPages,
  total: data.length,
  limit: 100,
});
const stock = (locationId: string, quantityPieces: number, productId = 'chips') => ({
  locationId,
  locationName: locationId,
  productId,
  quantityPieces,
  updatedAt: '',
});
const clients: QueryClient[] = [];
function mount(order = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  clients.push(client);
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[`/product-transactions/new${order ? '?orderId=order' : ''}`]}>
        <CreateProductTransactionPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return client;
}
async function switchStore(name: string) {
  fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Store' }));
  fireEvent.click(await screen.findByRole('option', { name }));
}
beforeEach(() => {
  vi.mocked(getVenueLocations).mockResolvedValue([
    {
      id: 'venue',
      organizationId: 'org',
      slug: 'venue',
      name: 'Venue',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      isActive: true,
    },
  ]);
  vi.mocked(getDeviceById).mockResolvedValue({
    id: 'd',
    name: 'Device',
    locationId: 'venue',
  } as never);
  vi.mocked(getInventoryLocations).mockResolvedValue(
    page([
      { id: 'a', name: 'Store A', kind: 'store', isActive: true, createdAt: '', updatedAt: '' },
      { id: 'b', name: 'Store B', kind: 'store', isActive: true, createdAt: '', updatedAt: '' },
    ]),
  );
  vi.mocked(getProducts).mockResolvedValue(
    page([product('chips', 'DEMO Chips'), product('burger', 'Burger')]),
  );
  vi.mocked(getLocationStock).mockImplementation(async (filters) =>
    page(filters?.locationId === 'a' ? [stock('a', 4)] : []),
  );
  vi.mocked(getCurrentProductPrices).mockImplementation(async (id) => [
    { productId: 'chips', price: 50, hasOptions: false },
    {
      productId: 'burger',
      price: 100,
      hasOptions: false,
      madeToOrderAvailable: id === 'a' ? 2 : 0,
    },
  ]);
  vi.mocked(getKioskOrder).mockResolvedValue({
    id: 'order',
    sessionId: 's',
    playerId: 'player',
    deviceId: 'd',
    status: 'pending',
    createdAt: '',
    updatedAt: '',
    lineItems: [
      { id: 'line', productId: 'chips', productName: 'DEMO Chips', quantity: 6, unitPrice: 50 },
    ],
  });
});
afterEach(() => {
  cleanup();
  for (const client of clients) client.clear();
  clients.length = 0;
  vi.resetAllMocks();
});

it('searches products when a catalog description is null', async () => {
  vi.mocked(getProducts).mockResolvedValue(
    page([{ ...product('chips', 'DEMO Chips'), description: null } as unknown as ProductResponse]),
  );
  mount();
  await screen.findByRole('button', { name: /DEMO Chips/ });
  fireEvent.change(screen.getByPlaceholderText('Search products'), {
    target: { value: 'chips' },
  });
  expect(screen.getByRole('button', { name: /DEMO Chips/ })).toBeInTheDocument();
});

it('shows only selected-location stock and recipe capacity, with zero for absent stock rows', async () => {
  mount();
  const chips = await screen.findByRole('button', { name: /DEMO Chips/ });
  expect(within(chips).getByText('4 pcs in store')).toBeInTheDocument();
  expect(screen.getByText('2 can be made')).toBeInTheDocument();
  expect(screen.queryByText('99 pcs in store')).not.toBeInTheDocument();
  fireEvent.click(chips);
  expect(screen.getByText('Cart (1 items)')).toBeInTheDocument();
  await switchStore('Store B');
  expect(screen.getByText('Cart (0 items)')).toBeInTheDocument();
  const otherChips = await screen.findByRole('button', { name: /DEMO Chips/ });
  expect(otherChips).toBeDisabled();
  expect(within(otherChips).getByText('0 pcs in store')).toBeInTheDocument();
  expect(screen.getByText('0 can be made')).toBeInTheDocument();
  expect(getCurrentProductPrices).toHaveBeenLastCalledWith('b', 'venue');
});

it('waits for every stock page and never exposes global quantities while loading', async () => {
  let resolveStock:
    | ((value: ReturnType<typeof page<ReturnType<typeof stock>>>) => void)
    | undefined;
  const pending = new Promise<ReturnType<typeof page<ReturnType<typeof stock>>>>((resolve) => {
    resolveStock = resolve;
  });
  vi.mocked(getLocationStock).mockImplementation(async (filters) =>
    filters?.page === 1 ? page([], 1, 2) : pending,
  );
  mount();
  await waitFor(() =>
    expect(getLocationStock).toHaveBeenCalledWith({ locationId: 'a', page: 2, limit: 100 }),
  );
  expect(
    screen.getByRole('progressbar', { name: 'Loading selected store stock' }),
  ).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /DEMO Chips/ })).not.toBeInTheDocument();
  await act(async () => resolveStock?.(page([stock('a', 8)], 2, 2)));
  expect(await screen.findByText('8 pcs in store')).toBeInTheDocument();
});

it('blocks sales on stock-load failure and allows an explicit retry', async () => {
  vi.mocked(getLocationStock).mockRejectedValueOnce(new Error('Stock unavailable'));
  mount(true);
  await screen.findByText(/Could not load stock and prices for Store A/);
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeDisabled();
  expect(screen.queryByText('99 pcs in store')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
  await screen.findByText('4 pcs in store');
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeDisabled();
  expect(addTransaction).not.toHaveBeenCalled();
});

it('revalidates a retained kiosk cart against current stock after location changes and refreshes', async () => {
  vi.mocked(getLocationStock).mockImplementation(async (filters) =>
    page([stock(String(filters?.locationId), filters?.locationId === 'a' ? 8 : 2)]),
  );
  const client = mount(true);
  await screen.findByText('8 pcs in store');
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeEnabled();
  await switchStore('Store B');
  await screen.findByText('2 pcs in store');
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeDisabled();
  expect(screen.getByText(/DEMO Chips: only 2 available at Store B/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Decrease quantity for DEMO Chips' }));
  expect(screen.getByRole('textbox', { name: 'Quantity for DEMO Chips' })).toHaveValue('5');
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeDisabled();
  fireEvent.change(screen.getByRole('textbox', { name: 'Quantity for DEMO Chips' }), {
    target: { value: '2' },
  });
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeEnabled();
  vi.mocked(getLocationStock).mockResolvedValue(page([stock('b', 1)]));
  await act(async () => {
    await client.invalidateQueries({ queryKey: ['pos-catalog', 'b'] });
  });
  await screen.findByText(/DEMO Chips: only 1 available at Store B/);
  expect(screen.getByRole('button', { name: 'Complete sale' })).toBeDisabled();
  expect(addTransaction).not.toHaveBeenCalled();
});
