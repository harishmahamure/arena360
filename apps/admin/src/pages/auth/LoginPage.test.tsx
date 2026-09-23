import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { loginPanelAPI, verifyPanelMfaAPI } from '../../services/auth/auth';
import { StoreContext } from '../../store';
import { rootInitialState } from '../../store/rootReducer';
import LoginPage from './LoginPage';

vi.mock('../../services/auth/auth', () => ({
  loginPanelAPI: vi.fn(),
  verifyPanelMfaAPI: vi.fn(),
}));

function LocationProbe() {
  return <span data-testid="location">{useLocation().pathname}</span>;
}

function renderLogin() {
  const dispatch = vi.fn();
  render(
    <MemoryRouter initialEntries={['/login']}>
      <StoreContext value={{ state: rootInitialState, dispatch }}>
        <LoginPage />
        <LocationProbe />
      </StoreContext>
    </MemoryRouter>,
  );
  return { dispatch };
}

describe('unified panel login', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  it('has no account-type selector and advances to the MFA challenge', async () => {
    vi.mocked(loginPanelAPI).mockResolvedValue({
      status: 'mfa_required',
      challengeToken: 'opaque-challenge',
      expiresAt: new Date(Date.now() + 300_000).toISOString(),
    });
    renderLogin();

    expect(screen.queryByText(/admin.*staff|staff.*admin/i)).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/Username/), { target: { value: 'operator' } });
    fireEvent.change(screen.getByLabelText(/Password/), { target: { value: 'secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    expect(await screen.findByText('Verify your identity')).toBeInTheDocument();
    expect(loginPanelAPI).toHaveBeenCalledWith('operator', 'secret');
  });

  it('routes authenticated staff to explicit shift setup after MFA', async () => {
    vi.mocked(loginPanelAPI).mockResolvedValue({
      status: 'mfa_required',
      challengeToken: 'opaque-challenge',
      expiresAt: new Date(Date.now() + 300_000).toISOString(),
    });
    vi.mocked(verifyPanelMfaAPI).mockResolvedValue({
      status: 'authenticated',
      accessToken: 'token',
      nextStep: 'shift_setup',
      user: {
        id: '1',
        username: 'operator',
        firstName: 'Counter',
        lastName: 'Staff',
        role: 'staff',
        isActive: true,
      },
    });
    renderLogin();
    fireEvent.change(screen.getByLabelText(/Username/), { target: { value: 'operator' } });
    fireEvent.change(screen.getByLabelText(/Password/), { target: { value: 'secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    await screen.findByText('Verify your identity');
    fireEvent.change(screen.getByLabelText(/Authenticator code/), {
      target: { value: '123456' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Verify and continue' }));

    await waitFor(() => expect(screen.getByTestId('location')).toHaveTextContent('/shift/setup'));
    expect(verifyPanelMfaAPI).toHaveBeenCalledWith('opaque-challenge', '123456');
  });
});
