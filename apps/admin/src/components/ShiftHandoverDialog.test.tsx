import '@testing-library/jest-dom/vitest';
import { local } from '@gaming-cafe/utils';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { rememberLocation, selectedLocationId } from '../lib/locationSelection';
import { closeShift, getExpectedClosing, handoverShift } from '../services/shifts';
import { StoreContext } from '../store';
import { authInitialState } from '../store/auth/action';
import ShiftHandoverDialog from './ShiftHandoverDialog';

vi.mock('../services/shifts', () => ({
  closeShift: vi.fn(),
  getExpectedClosing: vi.fn(),
  handoverShift: vi.fn(),
}));
function setup() {
  const onClose = vi.fn();
  render(
    <MemoryRouter>
      <StoreContext
        value={{
          state: { auth: { ...authInitialState, id: 'staff', role: 'staff' } },
          dispatch: vi.fn(),
        }}
      >
        <QueryClientProvider
          client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
        >
          <ShiftHandoverDialog open onClose={onClose} />
        </QueryClientProvider>
      </StoreContext>
    </MemoryRouter>,
  );
  return onClose;
}
beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  sessionStorage.clear();
  vi.mocked(getExpectedClosing).mockResolvedValue({ expectedClosing: 0, openingBalance: 0 });
});
afterEach(cleanup);
describe('staff shift closure', () => {
  it('requires an explicit count and permits a zero-cash drawer', async () => {
    vi.mocked(closeShift).mockResolvedValue({
      closedShift: {
        id: 'shift',
        userId: 'staff',
        status: 'closed',
        clockIn: '',
        createdAt: '',
        updatedAt: '',
      },
    });
    const onClose = setup();
    fireEvent.click(screen.getByRole('button', { name: /Close shift \(no replacement\)/ }));
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Closing Balance (optional if using denominations)'), {
      target: { value: '0' },
    });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
    expect(closeShift).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Close Shift' }));
    await waitFor(() =>
      expect(closeShift).toHaveBeenCalledWith(expect.objectContaining({ closingBalance: 0 })),
    );
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
  it('blocks closure when the expected drawer balance could not be verified', async () => {
    vi.mocked(getExpectedClosing).mockRejectedValue(new Error('Offline'));
    setup();
    await screen.findByText(/Could not verify the drawer balance/);
    fireEvent.click(screen.getByRole('button', { name: /Close shift \(no replacement\)/ }));
    fireEvent.change(screen.getByLabelText('Closing Balance (optional if using denominations)'), {
      target: { value: '10' },
    });
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
    expect(closeShift).not.toHaveBeenCalled();
  });
});

it('carries the operating location to the replacement operator during handover', async () => {
  const token = (userId: string) =>
    `header.${btoa(JSON.stringify({ userId, tenantId: 'tenant', roles: ['staff'], permissions: ['shifts:write', 'stats:read'], exp: Date.now() / 1000 + 3600 }))}.signature`;
  local.set('accessToken', token('outgoing'));
  rememberLocation('north');
  vi.mocked(handoverShift).mockResolvedValue({
    closedShift: { id: 'old-shift' },
    newAccessToken: token('incoming'),
    newUser: { id: 'incoming', username: 'next-operator', role: 'staff', isActive: true },
    newShiftId: 'new-shift',
  } as Awaited<ReturnType<typeof handoverShift>>);
  const onClose = setup();
  fireEvent.click(screen.getByRole('button', { name: /Handover to next staff/ }));
  fireEvent.change(screen.getByLabelText('Closing Balance (optional if using denominations)'), {
    target: { value: '0' },
  });
  await waitFor(() => expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled());
  fireEvent.click(screen.getByRole('button', { name: 'Next' }));
  fireEvent.click(screen.getByRole('button', { name: 'Next' }));
  fireEvent.change(screen.getByLabelText('Validator username'), {
    target: { value: 'next-operator' },
  });
  fireEvent.change(screen.getByLabelText('Validator password'), {
    target: { value: 'long-enough-password' },
  });
  fireEvent.change(screen.getByLabelText('Authenticator code'), { target: { value: '123456' } });
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  fireEvent.click(screen.getByRole('button', { name: 'Complete Handover' }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(selectedLocationId()).toBe('north');
});
