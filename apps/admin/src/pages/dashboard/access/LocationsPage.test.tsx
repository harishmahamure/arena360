import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createMember, getAccess, saveRole } from '../../../services/access';
import { createVenueLocation, getVenueLocations } from '../../../services/config';
import LocationsPage from './LocationsPage';

vi.mock('../../../hooks/usePermissions', () => ({
  Permission: { AccessManage: 'access:manage', LocationsManage: 'locations:manage' },
  usePermissions: () => ({ can: () => true }),
}));
vi.mock('../../../services/access', () => ({
  getAccess: vi.fn(),
  saveRole: vi.fn(),
  createMember: vi.fn(),
}));
vi.mock('../../../services/config', () => ({
  getVenueLocations: vi.fn(),
  createVenueLocation: vi.fn(),
  updateVenueLocation: vi.fn(),
}));

const venue = {
  id: '11111111-1111-4111-8111-111111111111',
  organizationId: '00000000-0000-4000-8000-000000000001',
  slug: 'north',
  name: 'North venue',
  timezone: 'Asia/Kolkata',
  currency: 'INR',
  isActive: true,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getVenueLocations).mockResolvedValue([venue]);
  vi.mocked(createVenueLocation).mockResolvedValue({
    ...venue,
    id: 'new',
    slug: 'south',
    name: 'South venue',
  });
  vi.mocked(getAccess).mockResolvedValue({
    catalog: [],
    roles: [],
    members: [],
    modules: [],
    audit: [],
  });
  vi.mocked(saveRole).mockResolvedValue({ id: 'manager-role' });
  vi.mocked(createMember).mockResolvedValue({ id: 'new-member' });
});
afterEach(cleanup);

it('shows location creation and scopes a new admin to the selected location', async () => {
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <MemoryRouter>
        <LocationsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );

  expect(screen.getByRole('link', { name: 'Manage existing team access' })).toHaveAttribute(
    'href',
    '/access?tab=team',
  );
  fireEvent.click(screen.getByRole('button', { name: 'Create location' }));
  fireEvent.change(screen.getByRole('textbox', { name: /Location name/ }), {
    target: { value: 'South venue' },
  });
  fireEvent.change(screen.getByRole('textbox', { name: /Slug/ }), { target: { value: 'south' } });
  fireEvent.click(screen.getByRole('button', { name: 'Add location' }));
  await waitFor(() =>
    expect(createVenueLocation).toHaveBeenCalledWith(
      venue.organizationId,
      expect.objectContaining({ name: 'South venue', slug: 'south' }),
    ),
  );

  fireEvent.click(await screen.findByRole('button', { name: 'Add location admin' }));
  fireEvent.change(screen.getByRole('textbox', { name: /Username/ }), {
    target: { value: 'north.admin' },
  });
  fireEvent.change(screen.getByLabelText(/Initial password/), {
    target: { value: 'ExamplePass1234' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Create admin' }));
  await waitFor(() =>
    expect(createMember).toHaveBeenCalledWith(
      expect.objectContaining({
        username: 'north.admin',
        roleIds: ['manager-role'],
        locationIds: [venue.id],
        locationRoles: [{ locationId: venue.id, roleIds: ['manager-role'] }],
      }),
    ),
  );
  expect(saveRole).toHaveBeenCalledWith(
    expect.objectContaining({ name: 'Location administrator', isTemplate: false }),
  );
});
