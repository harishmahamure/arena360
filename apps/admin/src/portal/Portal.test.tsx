import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Portal from './Portal';

const tenant = {
  id: 't1',
  name: 'Northside Gaming',
  slug: 'northside',
  state: 'ACTIVE',
  timezone: 'UTC',
  owner_cell: 'c1',
  cell_name: 'Local cell',
  schema_version: 17,
  ownership_generation: 1,
  lease_fresh: true,
  lease_expires_at: null,
  created_at: '2026-10-09T00:00:00Z',
};
const overview = {
  counts: { total: 1, active: 1, cold: 0, attention: 0 },
  localCellId: 'c1',
  canProvision: true,
  targetSchemaVersion: 17,
};
function mockApi(fail = false) {
  const fetch = vi.fn(async (url: string, options?: RequestInit) => {
    if (fail)
      return new Response(JSON.stringify({ message: 'Invalid platform operator token' }), {
        status: 401,
      });
    const path = url.split('?')[0] ?? '';
    let data: unknown = {};
    if (path.endsWith('/overview')) data = overview;
    else if (path.endsWith('/cells'))
      data = [
        {
          id: 'c1',
          name: 'Local cell',
          state: 'ACTIVE',
          tenant_count: 1,
          address: 'http://localhost:3000',
          hydration_ready: false,
        },
      ];
    else if (path.endsWith('/tenants'))
      data = options?.method === 'POST' ? { id: 't1' } : { items: [tenant] };
    else if (path.endsWith('/tenants/t1')) data = { tenant, admins: [], licenses: [], jobs: [] };
    return new Response(JSON.stringify(data), { status: 200 });
  });
  vi.stubGlobal('fetch', fetch);
  return fetch;
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
async function login() {
  fireEvent.change(screen.getByLabelText('Operator token'), {
    target: { value: 'operator-secret' },
  });
  fireEvent.click(screen.getByRole('button', { name: /Connect to platform/ }));
  await screen.findByText('Tenant directory');
  await screen.findByText('Northside Gaming');
}
describe('tenant management portal', () => {
  it('requires successful operator authentication and displays connection failures', async () => {
    mockApi(true);
    render(<Portal />);
    fireEvent.change(screen.getByLabelText('Operator token'), { target: { value: 'bad' } });
    fireEvent.click(screen.getByRole('button', { name: /Connect to platform/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('Invalid platform');
    expect(screen.queryByText('Tenant directory')).toBeNull();
  });
  it('uses operator bearer authentication, creates a tenant and offers administrator setup', async () => {
    const fetch = mockApi();
    render(<Portal />);
    await login();
    fireEvent.click(screen.getByRole('button', { name: '+ Create tenant' }));
    const dialog = screen.getByRole('dialog', { name: 'Create tenant' });
    fireEvent.change(within(dialog).getByLabelText('Tenant name'), {
      target: { value: 'New tenant' },
    });
    fireEvent.change(within(dialog).getByLabelText('Slug'), { target: { value: 'new-tenant' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create tenant' }));
    await waitFor(() =>
      expect(
        fetch.mock.calls.some(
          ([url, options]) =>
            url.endsWith('/tenants') &&
            options?.method === 'POST' &&
            JSON.parse(String(options.body)).slug === 'new-tenant',
        ),
      ).toBe(true),
    );
    await screen.findByRole('dialog', { name: 'Tenant details' });
    await screen.findByRole('button', { name: '+ Create administrator' });
    expect(
      fetch.mock.calls.every(
        ([, options]) =>
          (options?.headers as Record<string, string>).Authorization === 'Bearer operator-secret',
      ),
    ).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Close tenant details' }));
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect ↗' }));
    await screen.findByLabelText('Operator token');
  });
  it('shows real cell readiness separately from registration', async () => {
    mockApi();
    render(<Portal />);
    await login();
    fireEvent.click(screen.getByRole('button', { name: /Cells/ }));
    await screen.findByText('No fresh heartbeat');
    expect(screen.getByText('Connected cell')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Tenants/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Manage Northside Gaming' }));
    await screen.findByRole('button', { name: 'Put into cold storage' });
    expect(
      screen.getByRole('button', { name: 'Put into cold storage' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(screen.getByRole('button', { name: 'Move to cell' }).hasAttribute('disabled')).toBe(
      true,
    );
    expect(screen.getByText(/Move and cold storage need/)).toBeTruthy();
  });
});
