import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Portal from './Portal';

const tenant = {
  id: 't1',
  name: 'Northside Gaming',
  slug: 'northside',
  state: 'ACTIVE',
  is_enabled: true,
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
  targetSchemaVersion: 18,
  provisioningMode: 'remote',
  provisioningAddress: 'http://localhost:3001',
};
function mockApi(
  fail = false,
  firstLogin = false,
  config: { missingWorker?: boolean; inactiveTrial?: boolean; adminFailure?: boolean } = {},
) {
  let adminAttempts = 0;
  const fetch = vi.fn(async (url: string, options?: RequestInit) => {
    if (fail)
      return new Response(JSON.stringify({ message: 'Invalid credentials' }), {
        status: 401,
      });
    const path = url.split('?')[0] ?? '';
    let data: unknown = {};
    if (path.endsWith('/auth/login')) data = { challenge: 'challenge', setupRequired: firstLogin };
    else if (path.endsWith('/auth/totp/setup'))
      data = {
        secret: 'TESTSECRET',
        otpauthUri: 'otpauth://totp/Arena:operator?secret=TESTSECRET',
      };
    else if (path.endsWith('/auth/totp/verify')) data = { token: 'session' };
    else if (path.endsWith('/overview')) data = overview;
    else if (path.endsWith('/plans'))
      data = [
        {
          code: 'trial',
          name: 'Trial',
          graceDays: 7,
          entitlements: {},
          isActive: !config.inactiveTrial,
        },
      ];
    else if (path.endsWith('/cells'))
      data = config.missingWorker
        ? []
        : [
            {
              id: 'c1',
              name: 'Local cell',
              state: 'ACTIVE',
              tenant_count: 1,
              address: 'http://localhost:3000',
              hydration_ready: false,
            },
          ];
    else if (path.endsWith('/tenants/t1/admins')) {
      adminAttempts++;
      if (config.adminFailure && adminAttempts === 1)
        return new Response(
          JSON.stringify({ details: { message: 'Username is already in use' } }),
          { status: 409 },
        );
      data = { id: 'u1' };
    } else if (path.endsWith('/tenants'))
      data = options?.method === 'POST' ? { id: 't1' } : { items: [tenant] };
    else if (path.endsWith('/tenants/t1'))
      data = { tenant, admins: [], licenses: [], jobs: [], subscription: null };
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
  fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'operator' } });
  fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'operator-secret' } });
  fireEvent.click(screen.getByRole('button', { name: /Continue/ }));
  await screen.findByLabelText('Authentication code');
  fireEvent.change(screen.getByLabelText('Authentication code'), { target: { value: '123456' } });
  fireEvent.click(screen.getByRole('button', { name: /Verify and continue/ }));
  await screen.findByText('Tenant directory');
  await screen.findByText('Northside Gaming');
}
async function startTenant() {
  fireEvent.click(screen.getByRole('button', { name: '+ Create tenant' }));
  const dialog = screen.getByRole('dialog', { name: 'Create tenant' });
  fireEvent.change(within(dialog).getByLabelText(/^Tenant name/), {
    target: { value: 'New tenant' },
  });
  fireEvent.click(within(dialog).getByRole('button', { name: 'Create tenant' }));
  return screen.findByRole('dialog', { name: 'Create tenant administrator' });
}
async function finishAdministrator() {
  const dialog = await screen.findByRole('dialog', { name: 'Create tenant administrator' });
  expect((within(dialog).getByLabelText(/^Administrator username/) as HTMLInputElement).value).toBe(
    'new-tenant.admin',
  );
  fireEvent.change(within(dialog).getByLabelText(/^Administrator password/), {
    target: { value: 'new-admin-password' },
  });
  fireEvent.change(within(dialog).getByLabelText(/^Confirm password/), {
    target: { value: 'new-admin-password' },
  });
  fireEvent.click(within(dialog).getByRole('button', { name: 'Create administrator' }));
}
describe('tenant management portal', () => {
  it('requires TOTP enrollment on first login before showing management', async () => {
    const fetch = mockApi(false, true);
    render(<Portal />);
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'operator' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password' } });
    fireEvent.click(screen.getByRole('button', { name: /Continue/ }));
    await screen.findByText(/TESTSECRET/);
    expect(screen.queryByText('Tenant directory')).toBeNull();
    fireEvent.change(screen.getByLabelText('Authentication code'), { target: { value: '123456' } });
    fireEvent.click(screen.getByRole('button', { name: /Verify and continue/ }));
    await screen.findByText('Tenant directory');
    expect(fetch.mock.calls.some(([url]) => url.endsWith('/auth/totp/setup'))).toBe(true);
  });
  it('requires successful operator authentication and displays connection failures', async () => {
    mockApi(true);
    render(<Portal />);
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'operator' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'bad' } });
    fireEvent.click(screen.getByRole('button', { name: /Continue/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('Invalid credentials');
    expect(screen.queryByText('Tenant directory')).toBeNull();
  });
  it('uses operator session authentication, creates a tenant and offers administrator setup', async () => {
    const fetch = mockApi();
    render(<Portal />);
    await login();
    fireEvent.click(screen.getByRole('button', { name: '+ Create tenant' }));
    const dialog = screen.getByRole('dialog', { name: 'Create tenant' });
    fireEvent.change(within(dialog).getByLabelText(/^Tenant name/), {
      target: { value: 'New tenant' },
    });
    expect((within(dialog).getByLabelText(/^Slug/) as HTMLInputElement).value).toBe('new-tenant');
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
    await finishAdministrator();
    const done = await screen.findByRole('dialog', { name: 'Tenant setup complete' });
    expect(
      within(done)
        .getByRole('link', { name: /Open business admin/ })
        .getAttribute('href'),
    ).toBe('http://localhost:5173/');
    fireEvent.click(within(done).getByRole('button', { name: 'View tenant details' }));
    await screen.findByRole('dialog', { name: 'Tenant details' });
    await screen.findByRole('button', { name: '+ Create administrator' });
    expect(
      fetch.mock.calls.every(
        ([url, options]) =>
          url.endsWith('/auth/login') ||
          url.endsWith('/auth/totp/verify') ||
          (options?.headers as Record<string, string>).Authorization === 'Bearer session',
      ),
    ).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Close tenant details' }));
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect ↗' }));
    await screen.findByLabelText('Username');
  });
  it('shows real cell readiness separately from registration', async () => {
    mockApi();
    render(<Portal />);
    await login();
    fireEvent.click(screen.getByRole('button', { name: /Cells/ }));
    await screen.findByText('Not ready for restore');
    expect(screen.getByText('Default database worker')).toBeTruthy();
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
  it('retries only administrator creation after a failure', async () => {
    const fetch = mockApi(false, false, { adminFailure: true });
    render(<Portal />);
    await login();
    await startTenant();
    await finishAdministrator();
    expect((await screen.findByRole('alert')).textContent).toContain('Username is already in use');
    const dialog = screen.getByRole('dialog', { name: 'Create tenant administrator' });
    expect(within(dialog).getByText(/tenant is already saved/)).toBeTruthy();
    fireEvent.change(within(dialog).getByLabelText(/^Administrator username/), {
      target: { value: 'another.admin' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create administrator' }));
    await screen.findByRole('dialog', { name: 'Tenant setup complete' });
    const writes = fetch.mock.calls.filter(([, options]) => options?.method === 'POST');
    expect(writes.filter(([url]) => url.endsWith('/tenants'))).toHaveLength(1);
    expect(writes.filter(([url]) => url.endsWith('/admins'))).toHaveLength(2);
    const tenantWrite = writes.find(([url]) => url.endsWith('/tenants'));
    expect(JSON.parse(String(tenantWrite?.[1]?.body))).toEqual({
      name: 'New tenant',
      slug: 'new-tenant',
      timezone: 'Asia/Kolkata',
      trialDays: 30,
    });
  });
  it('can finish administrator setup later from tenant details', async () => {
    mockApi();
    render(<Portal />);
    await login();
    const dialog = await startTenant();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Finish later' }));
    const details = await screen.findByRole('dialog', { name: 'Tenant details' });
    await within(details).findByText(/Finish setup: create the tenant administrator/);
    fireEvent.click(within(details).getByRole('button', { name: '+ Create administrator' }));
    await screen.findByRole('dialog', { name: 'Create tenant administrator' });
  });
  it('blocks tenant creation and pre-fills the configured remote worker registration', async () => {
    mockApi(false, false, { missingWorker: true });
    render(<Portal />);
    await login();
    expect(screen.getByRole('button', { name: '+ Create tenant' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Register database worker' }));
    const dialog = screen.getByRole('dialog', { name: 'Register cell' });
    expect((within(dialog).getByLabelText('Cell ID') as HTMLInputElement).value).toBe('c1');
    expect(
      (within(dialog).getByLabelText('Database worker address') as HTMLInputElement).value,
    ).toBe('http://localhost:3001');
  });
  it('explains an inactive trial plan and opens its editor', async () => {
    mockApi(false, false, { inactiveTrial: true });
    render(<Portal />);
    await login();
    expect(screen.getByRole('button', { name: '+ Create tenant' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Review trial plan' }));
    await screen.findByRole('dialog', { name: 'Edit plan' });
  });
  it('validates timezone before sending a tenant creation request', async () => {
    const fetch = mockApi();
    render(<Portal />);
    await login();
    fireEvent.click(screen.getByRole('button', { name: '+ Create tenant' }));
    const dialog = screen.getByRole('dialog', { name: 'Create tenant' });
    fireEvent.change(within(dialog).getByLabelText(/^Tenant name/), {
      target: { value: 'New tenant' },
    });
    fireEvent.change(within(dialog).getByLabelText(/^Timezone/), {
      target: { value: 'Mars/Olympus' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create tenant' }));
    expect((await within(dialog).findByRole('alert')).textContent).toContain('valid timezone');
    expect(
      fetch.mock.calls.filter(
        ([url, options]) => url.endsWith('/tenants') && options?.method === 'POST',
      ),
    ).toHaveLength(0);
  });
});
