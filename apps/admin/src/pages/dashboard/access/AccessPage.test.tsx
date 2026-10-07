import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { getAccess, saveModule, saveRole } from '../../../services/access';
import AccessPage from './AccessPage';

vi.mock('../../../hooks/usePermissions', () => ({
  Permission: { AccessManage: 'access:manage' },
  usePermissions: () => ({ can: () => true }),
}));
vi.mock('../../../services/access', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../services/access')>()),
  getAccess: vi.fn(),
  saveRole: vi.fn(),
  saveModule: vi.fn(),
}));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getAccess).mockResolvedValue({
    catalog: [
      {
        key: 'kitchen',
        name: 'Kitchen',
        required: false,
        permissions: [
          { key: 'kitchen:read', label: 'Kitchen read', description: '' },
          { key: 'kitchen:write', label: 'Kitchen write', description: '' },
        ],
      },
    ],
    roles: [
      {
        id: 'assigned',
        name: 'Assigned role',
        description: '',
        permissions: ['kitchen:read'],
        revision: 1,
        isTemplate: false,
        memberCount: 1,
      },
    ],
    members: [],
    modules: [],
    audit: [],
  });
});
afterEach(cleanup);
function mount() {
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <MemoryRouter>
        <AccessPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
it('validates details and waits for review before saving explicit module grants', async () => {
  mount();
  await screen.findByText('Assigned role');
  fireEvent.click(screen.getByRole('button', { name: 'Create role' }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  expect(screen.getByText('Enter a role or template name.')).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText(/^Name/), { target: { value: 'Kitchen lead' } });
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  fireEvent.click(screen.getByRole('checkbox', { name: 'Kitchen write' }));
  expect(screen.getByRole('checkbox', { name: 'Kitchen read' })).toBeChecked();
  expect(saveRole).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save role' }));
  await waitFor(() =>
    expect(saveRole).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Kitchen lead',
        permissions: ['kitchen:write', 'kitchen:read'],
      }),
    ),
  );
});
it('protects assigned roles and reviews module changes before writing', async () => {
  mount();
  await screen.findByText('Assigned role');
  expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
  fireEvent.click(screen.getByRole('tab', { name: 'Modules' }));
  fireEvent.click(screen.getByRole('switch', { name: 'Kitchen enabled' }));
  expect(saveModule).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Confirm change' }));
  await waitFor(() =>
    expect(saveModule).toHaveBeenCalledWith({ key: 'kitchen', enabled: false, revision: 0 }),
  );
});
