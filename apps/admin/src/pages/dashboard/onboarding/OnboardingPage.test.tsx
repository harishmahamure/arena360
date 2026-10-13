import '@testing-library/jest-dom/vitest';
import { Permission } from '@gaming-cafe/contracts';
import { ApiError, http } from '@gaming-cafe/utils';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import OnboardingResumeCard from '../../../components/OnboardingResumeCard';
import type { OnboardingSnapshot, OnboardingStep } from '../../../services/onboarding';
import DashboardPage from '../DashboardPage';
import OnboardingPage from './OnboardingPage';

const context = vi.hoisted(() => ({ tenant: 'tenant-a', role: 'admin', write: true, stats: true }));
vi.mock('../AdminDashboardView', () => ({ default: () => <div>Dashboard</div> }));
vi.mock('../StaffDashboardView', () => ({ default: () => <div>Dashboard</div> }));
vi.mock('../../../hooks/usePermissions', () => ({
  usePermissions: () => ({
    role: context.role,
    can: (permission: Permission) =>
      permission === Permission.StatsRead
        ? context.stats
        : context.write || permission !== Permission.SettingsWrite,
  }),
}));
vi.mock('../../../lib/authSession', () => ({
  panelClaims: () => ({ tenantId: context.tenant, userId: 'actor' }),
  decodeJwtPayload: () => ({ tenantId: context.tenant }),
}));
vi.mock('@gaming-cafe/utils', async (original) => ({
  ...(await original<typeof import('@gaming-cafe/utils')>()),
  http: { get: vi.fn(), put: vi.fn(), post: vi.fn() },
}));

function snapshot(
  step: OnboardingStep = 'venue',
  revision = 0,
  started = false,
): OnboardingSnapshot {
  return {
    progress: {
      currentStep: step,
      revision,
      startedAt: started ? '2026-10-10T12:00:00Z' : null,
      updatedAt: null,
      settingsReviewedAt: null,
      completedAt: null,
    },
    readiness: {
      ready: false,
      checks: [
        { key: 'venue', label: 'Active venue', ready: true, actionPath: '/locations' },
        { key: 'operator', label: 'Counter operator', ready: true, actionPath: '/access' },
        {
          key: 'settings',
          label: 'Operating settings reviewed',
          ready: false,
          actionPath: '/onboarding',
        },
      ],
      locations: [],
    },
  };
}
let server: OnboardingSnapshot;
beforeEach(() => {
  vi.resetAllMocks();
  context.tenant = 'tenant-a';
  context.role = 'admin';
  context.write = true;
  context.stats = true;
  server = snapshot();
  vi.mocked(http.get).mockImplementation(async (path) => {
    if (path === '/onboarding') return structuredClone(server);
    if (path === '/access') return { members: [], roles: [], modules: [], catalog: [], audit: [] };
    if (path === '/devices') return { data: [], total: 0 };
    if (path.endsWith('/locations'))
      return server.readiness.locations.map((item) => ({ ...item, slug: 'arena', isActive: true }));
    if (path.includes('/settings/effective'))
      return [{ key: 'business.name', value: 'Arena cafe', revision: 0, sourceScope: 'platform' }];
    return [];
  });
});
afterEach(cleanup);

function setup(
  path = '/onboarding',
  client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  }),
) {
  const view = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/onboarding" element={<OnboardingPage />} />
          <Route
            path="/"
            element={
              <>
                <OnboardingResumeCard />
                <DashboardPage />
              </>
            }
          />
          <Route
            path="/devices/new"
            element={
              <>
                <OnboardingResumeCard />
                <div>Add a station</div>
              </>
            }
          />
          <Route path="/shift/setup" element={<div>Shift setup</div>} />
          <Route path="/access" element={<div>Access management</div>} />
          <Route path="/product-transactions" element={<div>Counter sales</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { ...view, client };
}

it('routes a fresh administrator to setup and lets them save and exit with a resume card', async () => {
  vi.mocked(http.put).mockImplementationOnce(async () => {
    server = snapshot('venue', 1, true);
    return structuredClone(server);
  });
  setup('/');
  expect(await screen.findByRole('heading', { name: 'Set up your venue' })).toBeInTheDocument();
  const exit = screen.getByRole('button', { name: 'Save and exit' });
  await waitFor(() => expect(exit).toBeEnabled());
  fireEvent.click(exit);
  expect(await screen.findByRole('link', { name: 'Resume setup' })).toBeInTheDocument();
  expect(http.put).toHaveBeenCalledWith('/onboarding', {
    currentStep: 'venue',
    expectedRevision: 0,
    reviewSettings: undefined,
  });
});

it('reloads the saved step after a full remount and resumes from another form', async () => {
  server = snapshot('stations', 7, true);
  const view = setup();
  expect(await screen.findByRole('textbox', { name: 'Station name' })).toBeInTheDocument();
  view.unmount();
  setup('/devices/new');
  fireEvent.click(await screen.findByRole('link', { name: 'Resume setup' }));
  expect(await screen.findByRole('textbox', { name: 'Plan name' })).toBeInTheDocument();
  expect(
    screen.getByText(
      'Set up one playable station and a time plan. You can add more after setup. Kiosk pairing is optional.',
    ),
  ).toBeInTheDocument();
});

it('retains the current step after a failed progress save and allows retry', async () => {
  server = snapshot('settings', 1, true);
  vi.mocked(http.put)
    .mockRejectedValueOnce(new Error('Connection lost'))
    .mockResolvedValueOnce(snapshot('stations', 2, true));
  setup();
  const next = await screen.findByRole('button', { name: 'Save and continue' });
  await waitFor(() => expect(next).toBeEnabled());
  fireEvent.click(next);
  expect(await screen.findByText('Connection lost')).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: 'Business name' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByRole('textbox', { name: 'Station name' })).toBeInTheDocument();
  expect(http.put).toHaveBeenLastCalledWith('/onboarding', {
    currentStep: 'stations',
    expectedRevision: 1,
    reviewSettings: true,
  });
});

it('reloads concurrent progress before sending a new revision', async () => {
  server = snapshot('settings', 3, true);
  vi.mocked(http.put).mockRejectedValueOnce(
    new ApiError({ message: 'ONBOARDING_REVISION_CONFLICT', statusCode: 409 }),
  );
  setup();
  const next = await screen.findByRole('button', { name: 'Save and continue' });
  await waitFor(() => expect(next).toBeEnabled());
  fireEvent.click(next);
  expect(await screen.findByText(/Another administrator updated setup/)).toBeInTheDocument();
  server = snapshot('stations', 4, true);
  server.readiness.locations = [
    {
      id: 'venue-id',
      name: 'Arena',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      stationCount: 1,
      compatiblePlanCount: 1,
      operatorReady: true,
    },
  ];
  fireEvent.click(screen.getByRole('button', { name: 'Reload saved progress' }));
  expect(await screen.findByRole('textbox', { name: 'Station name' })).toBeInTheDocument();
  vi.mocked(http.put).mockResolvedValue(snapshot('review', 5, true));
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  await waitFor(() =>
    expect(http.put).toHaveBeenLastCalledWith('/onboarding', {
      currentStep: 'review',
      expectedRevision: 4,
      reviewSettings: false,
    }),
  );
});

it('shows the location form immediately, keeps its draft on failure, and saves before advancing', async () => {
  setup();
  const name = await screen.findByRole('textbox', { name: 'Location name' });
  expect(screen.queryByRole('button', { name: 'Create location' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Refresh readiness' })).not.toBeInTheDocument();
  fireEvent.change(name, { target: { value: 'North venue' } });
  expect(screen.getByRole('textbox', { name: 'Location code' })).toHaveValue('north-venue');
  vi.mocked(http.post)
    .mockRejectedValueOnce(new Error('Location save failed'))
    .mockResolvedValueOnce({
      id: 'north',
      slug: 'north-venue',
      name: 'North venue',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      isActive: true,
    });
  vi.mocked(http.put).mockResolvedValue(snapshot('settings', 1, true));
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByText('Location save failed')).toBeInTheDocument();
  expect(name).toHaveValue('North venue');
  expect(http.put).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByRole('textbox', { name: 'Business name' })).toBeInTheDocument();
  expect(http.post).toHaveBeenLastCalledWith(
    '/organizations/tenant-a/locations',
    expect.objectContaining({ name: 'North venue', slug: 'north-venue' }),
  );
  expect(http.put).toHaveBeenLastCalledWith(
    '/onboarding',
    expect.objectContaining({ currentStep: 'settings' }),
  );
});

it('shows completion only after the server confirms and links to starting operations', async () => {
  server = snapshot('review', 4, true);
  server.readiness.ready = true;
  server.readiness.checks.forEach((check) => {
    check.ready = true;
  });
  const completed = structuredClone(server);
  completed.progress.completedAt = '2026-10-10T12:00:00Z';
  completed.progress.revision = 5;
  vi.mocked(http.post)
    .mockRejectedValueOnce(new ApiError({ message: 'ONBOARDING_NOT_READY', statusCode: 409 }))
    .mockResolvedValueOnce(completed);
  setup();
  fireEvent.click(await screen.findByRole('button', { name: 'Complete setup' }));
  expect(await screen.findByText(/Your venue changed since the last check/)).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Your venue is ready' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Complete setup' }));
  expect(await screen.findByRole('heading', { name: 'Your venue is ready' })).toBeInTheDocument();
  expect(http.post).toHaveBeenLastCalledWith('/onboarding/complete', { expectedRevision: 4 });
  fireEvent.click(screen.getByRole('link', { name: 'Start operations' }));
  expect(await screen.findByText('Shift setup')).toBeInTheDocument();
});

it('keeps completed administrators on their dashboard', async () => {
  server.progress.completedAt = '2026-10-10T12:00:00Z';
  setup('/');
  await waitFor(() => expect(http.get).toHaveBeenCalledWith('/onboarding'));
  expect(await screen.findByText('Dashboard')).toBeInTheDocument();
  expect(screen.queryByRole('link', { name: 'Resume setup' })).not.toBeInTheDocument();
});

it('checks first-login setup before redirecting an administrator without dashboard permission', async () => {
  context.stats = false;
  setup('/');
  expect(await screen.findByRole('heading', { name: 'Set up your venue' })).toBeInTheDocument();
  expect(screen.queryByText('Counter sales')).not.toBeInTheDocument();
});

it('preserves the normal home route for a completed administrator without dashboard permission', async () => {
  context.stats = false;
  server.progress.completedAt = '2026-10-10T12:00:00Z';
  setup('/');
  expect(await screen.findByText('Counter sales')).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Set up your venue' })).not.toBeInTheDocument();
});

it('shows current readiness changes when reopening completed setup', async () => {
  server.progress.completedAt = '2026-10-10T12:00:00Z';
  setup();
  expect(
    await screen.findByRole('heading', { name: 'Venue setup is complete' }),
  ).toBeInTheDocument();
  expect(
    screen.getByText(/Venue configuration has changed since setup was completed/),
  ).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Start operations' })).toHaveAttribute(
    'aria-disabled',
    'true',
  );
});

it('does not fetch admin setup for staff and disables saves for read-only administrators', async () => {
  context.role = 'staff';
  const view = setup('/');
  expect(screen.getByText('Dashboard')).toBeInTheDocument();
  expect(http.get).not.toHaveBeenCalled();
  view.unmount();
  context.role = 'admin';
  context.write = false;
  setup();
  expect(await screen.findByText(/Settings write permission is required/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Save and continue' })).toBeDisabled();
  expect(http.put).not.toHaveBeenCalled();
});

it('keeps tenant progress in separate query caches', async () => {
  server = snapshot('stations', 10, true);
  const { client, unmount } = setup();
  expect(await screen.findByRole('textbox', { name: 'Station name' })).toBeInTheDocument();
  unmount();
  context.tenant = 'tenant-b';
  server = snapshot('settings', 1, true);
  setup('/onboarding', client);
  expect(await screen.findByRole('textbox', { name: 'Business name' })).toBeInTheDocument();
  expect(screen.queryByRole('textbox', { name: 'Station name' })).not.toBeInTheDocument();
});

it('retries a failed plan save without creating the station twice and matches its location and equipment', async () => {
  server = snapshot('stations', 2, true);
  server.readiness.locations = [
    {
      id: 'venue-id',
      name: 'Arena',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      stationCount: 0,
      compatiblePlanCount: 0,
      operatorReady: true,
    },
  ];
  vi.mocked(http.post)
    .mockResolvedValueOnce({ id: 'station-id' })
    .mockRejectedValueOnce(new Error('Plan save failed'))
    .mockResolvedValueOnce({ id: 'plan-id' });
  vi.mocked(http.put).mockResolvedValue(snapshot('review', 3, true));
  setup();
  fireEvent.change(await screen.findByRole('textbox', { name: 'Station name' }), {
    target: { value: 'PC 01' },
  });
  fireEvent.change(screen.getByRole('textbox', { name: 'Plan name' }), {
    target: { value: 'One hour gaming' },
  });
  fireEvent.change(screen.getByRole('spinbutton', { name: 'Plan price (INR)' }), {
    target: { value: '120' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByText('Plan save failed')).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: 'Plan name' })).toHaveValue('One hour gaming');
  expect(http.put).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByRole('heading', { name: 'Review your venue' })).toBeInTheDocument();
  expect(vi.mocked(http.post).mock.calls.filter(([path]) => path === '/devices')).toHaveLength(1);
  expect(http.post).toHaveBeenLastCalledWith(
    '/plans',
    expect.objectContaining({
      locationIds: ['venue-id'],
      deviceType: 'PC',
      deviceSubType: 'HIGH_END_PCS',
      timeCredits: 60,
      price: 120,
      isActive: true,
    }),
  );
});

it('saves edited settings with their revisions and retains successful fields when a later save fails', async () => {
  server = snapshot('settings', 3, true);
  vi.mocked(http.get).mockImplementation(async (path) => {
    if (path === '/onboarding') return structuredClone(server);
    if (path.includes('/settings/effective'))
      return [
        { key: 'business.name', value: 'Arena cafe', revision: 2, sourceScope: 'organization' },
        { key: 'business.address', value: '', revision: 0, sourceScope: 'platform' },
      ];
    if (path === '/devices') return { data: [] };
    return [];
  });
  vi.mocked(http.put)
    .mockResolvedValueOnce({ value: 'New Arena', revision: 3 })
    .mockRejectedValueOnce(new Error('Address save failed'))
    .mockResolvedValueOnce({ value: 'North street', revision: 1 })
    .mockResolvedValueOnce(snapshot('stations', 4, true));
  setup();
  fireEvent.change(await screen.findByRole('textbox', { name: 'Business name' }), {
    target: { value: 'New Arena' },
  });
  fireEvent.change(screen.getByRole('textbox', { name: 'Business address' }), {
    target: { value: 'North street' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByText('Address save failed')).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: 'Business name' })).toHaveValue('New Arena');
  fireEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
  expect(await screen.findByRole('textbox', { name: 'Station name' })).toBeInTheDocument();
  expect(
    vi.mocked(http.put).mock.calls.filter(([path]) => path.endsWith('/business.name')),
  ).toHaveLength(1);
  expect(http.put).toHaveBeenCalledWith(
    '/organizations/tenant-a/settings/overrides/business.name',
    { value: 'New Arena', reason: 'Initial venue setup', expectedRevision: 2 },
  );
});

it('assigns an operator inside review, preserving their existing access and retrying without duplicate roles', async () => {
  server = snapshot('review', 4, true);
  server.readiness.checks.forEach((check) => {
    check.ready = check.key !== 'operator';
  });
  server.readiness.locations = [
    {
      id: 'venue-id',
      name: 'Arena',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      stationCount: 1,
      compatiblePlanCount: 1,
      operatorReady: false,
    },
  ];
  vi.mocked(http.get).mockImplementation(async (path) =>
    path === '/onboarding'
      ? structuredClone(server)
      : {
          members: [
            {
              id: 'staff-id',
              username: 'operator',
              name: 'Alex',
              active: true,
              revision: 7,
              roleIds: ['existing-role'],
              locationIds: ['other-venue'],
              locationRoles: [{ locationId: 'other-venue', roleIds: ['existing-role'] }],
            },
          ],
          roles: [
            {
              id: 'manager-role',
              name: 'Counter operator',
              description: 'Manager',
              permissions: Object.values(Permission),
              isTemplate: false,
              revision: 1,
              memberCount: 1,
            },
          ],
          modules: [],
          catalog: [],
          audit: [],
        },
  );
  vi.mocked(http.post).mockImplementation(async (path) =>
    path === '/access/roles'
      ? { id: 'counter-role' }
      : {
          ...server,
          readiness: { ...server.readiness, ready: true },
          progress: { ...server.progress, completedAt: '2026-10-10T12:00:00Z' },
        },
  );
  vi.mocked(http.put)
    .mockRejectedValueOnce(new Error('Operator save failed'))
    .mockResolvedValueOnce({});
  setup();
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: 'Counter operator' }));
  fireEvent.click(await screen.findByRole('option', { name: 'Alex' }));
  fireEvent.click(screen.getByRole('button', { name: 'Complete setup' }));
  expect(await screen.findByText('Operator save failed')).toBeInTheDocument();
  expect(
    vi.mocked(http.post).mock.calls.filter(([path]) => path === '/onboarding/complete'),
  ).toHaveLength(0);
  fireEvent.click(screen.getByRole('button', { name: 'Complete setup' }));
  expect(await screen.findByRole('heading', { name: 'Your venue is ready' })).toBeInTheDocument();
  expect(vi.mocked(http.post).mock.calls.filter(([path]) => path === '/access/roles')).toHaveLength(
    1,
  );
  expect(http.post).toHaveBeenCalledWith(
    '/access/roles',
    expect.objectContaining({
      name: 'Counter operator 2',
      permissions: expect.not.arrayContaining([Permission.AccessManage]),
    }),
  );
  expect(http.put).toHaveBeenLastCalledWith('/access/members/staff-id', {
    active: true,
    expectedRevision: 7,
    roleIds: ['existing-role', 'counter-role'],
    locationIds: ['other-venue', 'venue-id'],
    locationRoles: [
      { locationId: 'other-venue', roleIds: ['existing-role'] },
      { locationId: 'venue-id', roleIds: ['counter-role'] },
    ],
  });
});

it('keeps review navigation disabled for a read-only administrator', async () => {
  context.write = false;
  server = snapshot('review', 4, true);
  setup();
  expect(await screen.findByRole('button', { name: 'Edit' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Complete setup' })).toBeDisabled();
  expect(http.put).not.toHaveBeenCalled();
});
