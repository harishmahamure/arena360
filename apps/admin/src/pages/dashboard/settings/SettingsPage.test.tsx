import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as api from '../../../services/config';
import SettingsPage from './SettingsPage';

vi.mock('../../../hooks/usePermissions', () => ({
  Permission: { ConfigWrite: 'config:write' },
  usePermissions: () => ({ can: () => true }),
}));
vi.mock('./PricingRulesPanel', () => ({ default: () => <div>Pricing policy editor</div> }));
vi.mock('../../../services/config', () => ({
  getSettingCatalog: vi.fn(),
  getVenueLocations: vi.fn(),
  getEffectiveSettings: vi.fn(),
  getSettingHistory: vi.fn(),
  putSettingOverride: vi.fn(),
  deleteSettingOverride: vi.fn(),
}));
const fields: api.SettingDefinition[] = [
  {
    key: 'business.name',
    category: 'business',
    description: 'Venue name',
    valueType: 'string',
    defaultValue: 'Arena',
    allowedScopes: ['organization', 'location'],
    validation: {},
    sensitive: false,
    owner: 'venue',
  },
  {
    key: 'sessions.warning',
    category: 'sessions',
    description: 'Minutes before ending',
    valueType: 'integer',
    defaultValue: 5,
    allowedScopes: ['organization', 'location'],
    validation: { minimum: 0, maximum: 120 },
    sensitive: false,
    owner: 'venue',
  },
];
const effective: [api.ResolvedSetting, api.ResolvedSetting] = [
  {
    key: 'business.name',
    value: 'Arena',
    sourceScope: 'organization',
    revision: 4,
    overridden: true,
  },
  { key: 'sessions.warning', value: 5, sourceScope: 'platform', revision: 0, overridden: false },
];
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <SettingsPage />
      </QueryClientProvider>
    </MemoryRouter>,
  );
  return client;
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getSettingCatalog).mockResolvedValue(fields);
  vi.mocked(api.getEffectiveSettings).mockResolvedValue(effective);
  vi.mocked(api.getVenueLocations).mockResolvedValue([
    {
      id: 'loc-1',
      organizationId: 'org-1',
      name: 'North venue',
      slug: 'north',
      timezone: 'Asia/Kolkata',
      currency: 'INR',
      isActive: true,
    },
  ]);
  vi.mocked(api.getSettingHistory).mockResolvedValue([]);
  vi.mocked(api.putSettingOverride).mockResolvedValue({
    id: 'override',
    organizationId: 'org-1',
    key: 'business.name',
    value: 'Updated',
    revision: 5,
    updatedAt: '',
  });
});
afterEach(cleanup);
describe('configuration workflow', () => {
  it('keeps dirty edits and their original revision across background refreshes', async () => {
    const client = setup();
    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: 'Updated' } });
    vi.mocked(api.getEffectiveSettings).mockResolvedValue([
      { ...effective[0], value: 'External edit', revision: 9 },
      effective[1],
    ]);
    await client.invalidateQueries({ queryKey: ['effective-settings'] });
    expect(screen.getByLabelText('Name')).toHaveValue('Updated');
    fireEvent.click(screen.getByRole('button', { name: 'Review & save' }));
    fireEvent.change(screen.getByLabelText('Reason for this change'), {
      target: { value: 'Rename venue' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Apply changes' }));
    await waitFor(() =>
      expect(api.putSettingOverride).toHaveBeenCalledWith(
        expect.any(String),
        'business.name',
        expect.objectContaining({ value: 'Updated', expectedRevision: 4, reason: 'Rename venue' }),
      ),
    );
  });
  it('retains remaining edits when a later save fails', async () => {
    vi.mocked(api.putSettingOverride)
      .mockResolvedValueOnce({
        id: 'override',
        organizationId: 'org-1',
        key: 'business.name',
        value: 'Updated',
        revision: 5,
        updatedAt: '',
      })
      .mockRejectedValueOnce(new Error('Conflict'));
    setup();
    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    fireEvent.change(screen.getByLabelText('Warning'), { target: { value: '10' } });
    fireEvent.click(screen.getByRole('button', { name: 'Review & save' }));
    fireEvent.change(screen.getByLabelText('Reason for this change'), {
      target: { value: 'Update venue settings' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Apply changes' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByText('1 unsaved change')).toBeInTheDocument();
    expect(screen.getByLabelText('Warning')).toHaveValue(10);
  });
  it('asks before discarding changes on scope switch', async () => {
    setup();
    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: 'Updated' } });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Editing scope' }));
    fireEvent.click(await screen.findByRole('option', { name: 'North venue' }));
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('Discard unsaved changes?')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(screen.getByLabelText('Name')).toHaveValue('Updated');
  });
  it('shows recoverable load failures instead of empty settings', async () => {
    vi.mocked(api.getSettingCatalog).mockRejectedValue(new Error('Unavailable'));
    setup();
    expect(await screen.findByText(/Configuration could not be loaded/)).toBeInTheDocument();
    expect(screen.getByText(/Settings catalog: Unavailable/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument();
  });
});
