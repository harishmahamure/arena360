import '@testing-library/jest-dom/vitest';
import { http } from '@gaming-cafe/utils';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import CatalogLocationPanel from './CatalogLocationPanel';

const context = vi.hoisted(() => ({
  organizationAdmin: false,
  locations: [
    { id: 'north', name: 'North' },
    { id: 'south', name: 'South' },
  ],
}));
vi.mock('@gaming-cafe/utils', () => ({ http: { get: vi.fn(), put: vi.fn() } }));
vi.mock('./CatalogLocationFields', () => ({
  useCatalogLocations: () => context,
  default: () => null,
}));
afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  context.organizationAdmin = false;
});
function setup() {
  const onCanEditChange = vi.fn();
  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
        })
      }
    >
      <CatalogLocationPanel kind="products" id="item" canWrite onCanEditChange={onCanEditChange} />
    </QueryClientProvider>,
  );
  return onCanEditChange;
}
it('keeps business-wide shared fields read only for location admins', async () => {
  vi.mocked(http.get).mockResolvedValue({
    locationIds: [],
    prices: [],
    writableLocationIds: ['north', 'south'],
  });
  const changed = setup();
  expect(await screen.findByText(/organization administrator manages/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Save sharing and prices' })).toBeDisabled();
  expect(changed).toHaveBeenLastCalledWith(false);
});
it('saves an independent price and can restore the shared catalog price', async () => {
  vi.mocked(http.get).mockResolvedValue({
    locationIds: ['north', 'south'],
    prices: [{ locationId: 'north', price: 25 }],
    writableLocationIds: ['north', 'south'],
  });
  vi.mocked(http.put).mockResolvedValue({
    locationIds: ['north', 'south'],
    prices: [],
    writableLocationIds: ['north', 'south'],
  });
  setup();
  fireEvent.click(await screen.findByRole('button', { name: 'Use catalog price' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save sharing and prices' }));
  await waitFor(() =>
    expect(http.put).toHaveBeenCalledWith('/products/item/location-scope', {
      locationIds: ['north', 'south'],
      prices: [],
    }),
  );
});
it('does not allow editing a record shared with an unassigned location', async () => {
  vi.mocked(http.get).mockResolvedValue({
    locationIds: ['north', 'elsewhere'],
    prices: [],
    writableLocationIds: ['north', 'south'],
  });
  setup();
  expect(await screen.findByText(/organization administrator manages/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Save sharing and prices' })).toBeDisabled();
});

it('preserves read-only prices when saving another location price', async () => {
  vi.mocked(http.get).mockResolvedValue({
    locationIds: [],
    prices: [
      { locationId: 'north', price: 25 },
      { locationId: 'south', price: 40 },
    ],
    writableLocationIds: ['north'],
  });
  vi.mocked(http.put).mockResolvedValue({ locationIds: [], prices: [] });
  setup();
  fireEvent.click(await screen.findByRole('button', { name: 'Use catalog price' }));
  expect(screen.getByDisplayValue('40')).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: 'Save sharing and prices' }));
  await waitFor(() =>
    expect(http.put).toHaveBeenCalledWith('/products/item/location-scope', {
      locationIds: [],
      prices: [],
    }),
  );
});
