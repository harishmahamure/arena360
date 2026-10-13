import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { rememberLocation } from '../../lib/locationSelection';
import { getVenueLocations } from '../../services/config';
import { getShiftStartContext, startShift } from '../../services/shifts';
import ShiftSetupPage from './ShiftSetupPage';

vi.mock('../../hooks/usePermissions', () => ({ usePermissions: () => ({ isStaff: true }) }));
vi.mock('../../lib/locationSelection', () => ({ rememberLocation: vi.fn() }));
vi.mock('../dashboard/access/LocationsPanel', () => ({ currentOrganizationId: () => 'tenant' }));
vi.mock('../../services/config', () => ({ getVenueLocations: vi.fn() }));
vi.mock('../../services/shifts', () => ({ getShiftStartContext: vi.fn(), startShift: vi.fn() }));
vi.mock('@gaming-cafe/utils', async (original) => ({
  ...(await original<typeof import('@gaming-cafe/utils')>()),
  local: { get: () => 'token' },
  toastUtils: { success: vi.fn(), error: vi.fn() },
}));
const north = {
  id: 'north',
  organizationId: 'tenant',
  name: 'North venue',
  slug: 'north',
  timezone: 'Asia/Kolkata',
  currency: 'INR',
  isActive: true,
};
const south = { ...north, id: 'south', name: 'South venue', slug: 'south' };
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  client.setQueryData(['sessions'], ['old-location-session']);
  const view = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/shift/setup']}>
        <Routes>
          <Route path="/shift/setup" element={<ShiftSetupPage />} />
          <Route path="/" element={<div>Location dashboard</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { ...view, client };
}
async function choose(name: string) {
  fireEvent.click(await screen.findByRole('radio', { name: new RegExp(name) }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue to shift setup' }));
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getVenueLocations).mockResolvedValue([north, south]);
  vi.mocked(getShiftStartContext).mockResolvedValue({
    mode: 'start',
    suggestedOpeningBalance: 100,
  });
  vi.mocked(startShift).mockResolvedValue({
    resumed: false,
    shift: { id: 'shift' },
    cashRegister: { id: 'register' },
  } as Awaited<ReturnType<typeof startShift>>);
});
afterEach(cleanup);

it('asks for a location before fetching cash details or starting a shift, then scopes the dashboard to it', async () => {
  const { client } = setup();
  expect(
    await screen.findByRole('heading', { name: 'Choose your shift location' }),
  ).toBeInTheDocument();
  expect(getShiftStartContext).not.toHaveBeenCalled();
  expect(startShift).not.toHaveBeenCalled();
  expect(screen.queryByLabelText('Confirmed opening cash')).not.toBeInTheDocument();
  await choose('South venue');
  expect(await screen.findByRole('heading', { name: 'Set up your shift' })).toBeInTheDocument();
  expect(getShiftStartContext).toHaveBeenCalledWith('south');
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  fireEvent.click(screen.getByRole('button', { name: 'Start shift and open register' }));
  expect(await screen.findByText('Location dashboard')).toBeInTheDocument();
  expect(startShift).toHaveBeenCalledWith({
    openingBalance: 100,
    notes: undefined,
    venueLocationId: 'south',
  });
  expect(rememberLocation).toHaveBeenCalledWith('south');
  expect(client.getQueryData(['sessions'])).toBeUndefined();
});

it('selects the only available location automatically', async () => {
  vi.mocked(getVenueLocations).mockResolvedValue([north]);
  setup();
  expect(await screen.findByRole('heading', { name: 'Set up your shift' })).toBeInTheDocument();
  expect(getShiftStartContext).toHaveBeenCalledWith('north');
  expect(
    screen.queryByRole('heading', { name: 'Choose your shift location' }),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
});

it('does not switch operating location when shift creation fails', async () => {
  vi.mocked(startShift).mockRejectedValue(new Error('Shift could not start'));
  setup();
  await choose('North venue');
  fireEvent.click(await screen.findByRole('button', { name: 'Review details' }));
  const start = await screen.findByRole('button', { name: 'Start shift and open register' });
  fireEvent.click(start);
  await waitFor(() => expect(startShift).toHaveBeenCalled());
  await waitFor(() => expect(start).toBeEnabled());
  expect(rememberLocation).not.toHaveBeenCalled();
  expect(screen.queryByText('Location dashboard')).not.toBeInTheDocument();
});

it('lets a user correct a selection when their active shift belongs to another venue', async () => {
  vi.mocked(getShiftStartContext)
    .mockRejectedValueOnce(new Error('An active shift belongs to another location'))
    .mockResolvedValueOnce({ mode: 'resume', suggestedOpeningBalance: 100 });
  setup();
  await choose('North venue');
  expect(
    await screen.findByText(/An active shift belongs to another location/),
  ).toBeInTheDocument();
  expect(startShift).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Choose another location' }));
  await choose('South venue');
  expect(await screen.findByRole('heading', { name: 'Resume your shift' })).toBeInTheDocument();
  expect(getShiftStartContext).toHaveBeenLastCalledWith('south');
});
