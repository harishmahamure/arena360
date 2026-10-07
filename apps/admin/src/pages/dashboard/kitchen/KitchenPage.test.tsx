import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  advanceKitchenTicket,
  getKitchenMenu,
  getKitchenTickets,
  saveKitchenMenu,
} from '../../../services/operations';
import KitchenPage from './KitchenPage';

vi.mock('../../../hooks/usePermissions', () => ({
  Permission: { KitchenManage: 'kitchen:manage', KitchenWrite: 'kitchen:write' },
  usePermissions: () => ({ can: () => true }),
}));
vi.mock('../../../services/operations', () => ({
  getKitchenMenu: vi.fn(),
  getKitchenTickets: vi.fn(),
  advanceKitchenTicket: vi.fn(),
  saveKitchenMenu: vi.fn(),
}));
const ticket = {
  id: 'ticket-one',
  transactionId: 'sale-one',
  status: 'queued' as const,
  revision: 3,
  customer: 'Alex',
  notes: null,
  paymentStatus: 'completed',
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  dueAt: new Date(Date.now() + 60000).toISOString(),
  items: [{ productId: 'p1', name: 'Toast', quantity: 2, station: 'Hot kitchen' }],
  events: [],
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getKitchenTickets).mockResolvedValue([ticket]);
  vi.mocked(getKitchenMenu).mockResolvedValue([
    {
      productId: 'p1',
      name: 'Toast',
      enabled: false,
      station: 'Kitchen',
      prepMinutes: 15,
      revision: 0,
    },
  ]);
});
afterEach(cleanup);
function mount() {
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <MemoryRouter>
        <KitchenPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
it('requires a cancellation reason and sends the observed revision', async () => {
  mount();
  fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
  expect(screen.getByRole('button', { name: 'Cancel ticket' })).toBeDisabled();
  fireEvent.change(screen.getByLabelText('Reason'), {
    target: { value: 'Customer changed order' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Cancel ticket' }));
  await waitFor(() =>
    expect(advanceKitchenTicket).toHaveBeenCalledWith(
      ticket,
      'cancelled',
      'Customer changed order',
    ),
  );
});
it('holds kitchen configuration until the review step', async () => {
  mount();
  fireEvent.click(screen.getByRole('tab', { name: 'Menu setup' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Configure' }));
  fireEvent.click(screen.getByRole('switch', { name: 'Send to kitchen after sale' }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  fireEvent.change(screen.getByLabelText(/Prep station/), { target: { value: 'Grill' } });
  expect(saveKitchenMenu).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  expect(screen.getByText('Toast → Grill, 15 minute preparation target')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Save kitchen setup' }));
  await waitFor(() =>
    expect(vi.mocked(saveKitchenMenu).mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({ station: 'Grill', enabled: true, revision: 0 }),
    ),
  );
});
it('blocks preparation when payment has been refunded', async () => {
  vi.mocked(getKitchenTickets).mockResolvedValue([{ ...ticket, paymentStatus: 'refunded' }]);
  mount();
  expect(await screen.findByRole('button', { name: 'Start preparing' })).toBeDisabled();
});
