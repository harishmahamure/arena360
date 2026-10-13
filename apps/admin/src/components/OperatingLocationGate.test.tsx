import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { selectLocation } from '../lib/locationSelection';
import { getVenueLocations } from '../services/config';
import { getShiftStartContext } from '../services/shifts';
import OperatingLocationGate from './OperatingLocationGate';

const context = vi.hoisted(() => ({ selected: '', staff: false, fresh: false, token: 'token' }));
const operations = vi.hoisted(() => vi.fn(async () => 'Loaded operations'));
vi.mock('../hooks/usePermissions', () => ({
  usePermissions: () => ({ can: () => true, isStaff: context.staff }),
}));
vi.mock('../hooks/useOnboarding', () => ({
  useOnboarding: () => ({
    canWrite: true,
    isPending: false,
    data: { progress: { startedAt: context.fresh ? null : 'started', completedAt: null } },
  }),
}));
vi.mock('../lib/authSession', () => ({
  panelClaims: () => ({ tenantId: 'tenant' }),
  clearAdminSession: vi.fn(),
}));
vi.mock('@gaming-cafe/utils', () => ({ local: { get: () => context.token } }));
vi.mock('../lib/locationSelection', () => ({
  selectedLocationId: () => context.selected,
  selectLocation: vi.fn((id) => {
    context.selected = id;
  }),
}));
vi.mock('../services/config', () => ({ getVenueLocations: vi.fn() }));
vi.mock('../services/shifts', () => ({ getShiftStartContext: vi.fn() }));
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

function Dashboard() {
  useQuery({ queryKey: ['operations'], queryFn: operations });
  return <div>Dashboard operations</div>;
}
function setup(path = '/') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const tree = (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route
            path="/"
            element={
              <OperatingLocationGate>
                <Dashboard />
              </OperatingLocationGate>
            }
          />
          <Route
            path="/devices/new"
            element={
              <OperatingLocationGate>
                <Dashboard />
              </OperatingLocationGate>
            }
          />
          <Route
            path="/settings"
            element={
              <OperatingLocationGate>
                <div>Organization settings</div>
              </OperatingLocationGate>
            }
          />
          <Route path="/onboarding" element={<div>Venue wizard</div>} />
          <Route path="/login" element={<div>Sign in</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
  const view = render(tree);
  return { ...view, reload: () => view.rerender(tree) };
}
beforeEach(() => {
  vi.clearAllMocks();
  Object.assign(context, { selected: '', staff: false, fresh: false, token: 'token' });
  vi.mocked(getVenueLocations).mockResolvedValue([north, south]);
  vi.mocked(getShiftStartContext).mockResolvedValue({ mode: 'start', suggestedOpeningBalance: 0 });
});
afterEach(cleanup);

it('requires an explicit choice before the dashboard and its data queries mount', async () => {
  setup('/devices/new');
  expect(await screen.findByRole('heading', { name: 'Choose your location' })).toBeInTheDocument();
  expect(screen.queryByText('Dashboard operations')).not.toBeInTheDocument();
  expect(operations).not.toHaveBeenCalled();
  expect(getShiftStartContext).not.toHaveBeenCalled();
  expect(screen.queryByText('All accessible locations')).not.toBeInTheDocument();
  const next = screen.getByRole('button', { name: 'Continue to workspace' });
  expect(next).toBeDisabled();
  fireEvent.click(screen.getByRole('radio', { name: /South venue/ }));
  fireEvent.click(next);
  expect(selectLocation).toHaveBeenCalledWith('south');
});

it('automatically selects a sole active location before mounting operational content', async () => {
  vi.mocked(getVenueLocations).mockResolvedValue([north, { ...south, isActive: false }]);
  setup();
  await waitFor(() => expect(selectLocation).toHaveBeenCalledWith('north'));
  expect(screen.queryByRole('heading', { name: 'Choose your location' })).not.toBeInTheDocument();
  // Selection reloads the app; mount a fresh tree using the saved selection.
  cleanup();
  setup();
  expect(await screen.findByText('Dashboard operations')).toBeInTheDocument();
  await waitFor(() => expect(operations).toHaveBeenCalled());
});

it('validates a saved location and blocks a removed assignment instead of silently choosing the first', async () => {
  context.selected = 'removed';
  setup();
  expect(await screen.findByRole('heading', { name: 'Choose your location' })).toBeInTheDocument();
  expect(selectLocation).not.toHaveBeenCalled();
  expect(operations).not.toHaveBeenCalled();
});

it('checks an existing shift belongs to the selected location before operations load', async () => {
  context.selected = 'south';
  context.staff = true;
  vi.mocked(getShiftStartContext).mockRejectedValue(
    new Error('An active shift belongs to another location'),
  );
  setup();
  expect(
    await screen.findByText(/An active shift belongs to another location/),
  ).toBeInTheDocument();
  expect(getShiftStartContext).toHaveBeenCalledWith('south');
  expect(screen.getByRole('heading', { name: 'Choose your location' })).toBeInTheDocument();
  expect(operations).not.toHaveBeenCalled();
});

it('lets staff resume operations at a validated remembered location', async () => {
  context.selected = 'north';
  context.staff = true;
  vi.mocked(getShiftStartContext).mockResolvedValue({ mode: 'resume', suggestedOpeningBalance: 0 });
  setup();
  expect(await screen.findByText('Dashboard operations')).toBeInTheDocument();
  expect(getShiftStartContext).toHaveBeenCalledWith('north');
  expect(selectLocation).not.toHaveBeenCalled();
});

it('gives first-login venue setup priority and keeps organization settings reachable without locations', async () => {
  context.fresh = true;
  setup();
  expect(await screen.findByText('Venue wizard')).toBeInTheDocument();
  expect(getVenueLocations).not.toHaveBeenCalled();
  cleanup();
  setup('/settings');
  expect(await screen.findByText('Organization settings')).toBeInTheDocument();
  expect(getVenueLocations).not.toHaveBeenCalled();
});

it('blocks operations on a location fetch error and recovers through retry', async () => {
  vi.mocked(getVenueLocations)
    .mockRejectedValueOnce(new Error('Offline'))
    .mockResolvedValueOnce([north, south]);
  setup();
  expect(await screen.findByText(/Locations could not be loaded/)).toBeInTheDocument();
  expect(operations).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
  expect(await screen.findByRole('heading', { name: 'Choose your location' })).toBeInTheDocument();
});
