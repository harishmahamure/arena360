import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_FAILURES } from '../lib/loginLockout';
import { LoginHomePage } from './LoginHomePage';

const playerLogin = vi.fn();

vi.mock('../context/KioskProvider', () => ({
  useKiosk: () => ({
    playerLogin,
    error: null,
    online: true,
    maintenance: false,
    deviceName: 'PC-01',
  }),
}));

describe('LoginHomePage', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  it('renders the sign-in form', () => {
    render(<LoginHomePage />);
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/app version/i)).toHaveTextContent(/^v\d/);
  });

  it('locks the form after too many failures', () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<LoginHomePage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeDisabled();
  });
});
