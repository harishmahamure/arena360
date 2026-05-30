import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_FAILURES } from '../lib/loginLockout';
import { PlayerLoginPage } from './PlayerLoginPage';

const playerLogin = vi.fn();
const enterSetup = vi.fn();

vi.mock('../context/KioskProvider', () => ({
  useKiosk: () => ({ playerLogin, enterSetup, error: null }),
}));

describe('PlayerLoginPage', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  it('renders the sign-in form', () => {
    render(<PlayerLoginPage />);
    expect(screen.getByRole('heading', { name: /player sign in/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /start session/i })).toBeInTheDocument();
  });

  it('locks the form after too many failures', () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<PlayerLoginPage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /start session/i })).toBeDisabled();
  });
});
