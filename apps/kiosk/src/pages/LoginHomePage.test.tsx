import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SESSION_EXPIRED_DISMISS_MS, SESSION_EXPIRED_MESSAGE } from '../lib/authMessages';
import { MAX_FAILURES } from '../lib/loginLockout';
import { LoginHomePage } from './LoginHomePage';

const playerLogin = vi.fn();
const clearLoginNotice = vi.fn();
const clearError = vi.fn();
let loginNotice: string | null = null;
let error: string | null = null;

vi.mock('../context/KioskProvider', () => ({
  useKiosk: () => ({
    playerLogin,
    get error() {
      return error;
    },
    clearError,
    online: true,
    maintenance: false,
    deviceName: 'PC-01',
    get loginNotice() {
      return loginNotice;
    },
    clearLoginNotice,
  }),
}));

describe('LoginHomePage', () => {
  beforeEach(() => {
    localStorage.clear();
    loginNotice = null;
    error = null;
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('renders the sign-in form', () => {
    render(<LoginHomePage />);
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/app version/i)).toHaveTextContent(/^v\d/);
    expect(screen.queryByRole('button', { name: /^lock$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^restart$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^shutdown$/i })).not.toBeInTheDocument();
    expect(screen.getByLabelText(/station status/i)).toBeInTheDocument();
  });

  it('locks the form after too many failures', () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<LoginHomePage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeDisabled();
  });

  it('staff Ctrl+Shift+B clears login lockout', async () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<LoginHomePage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();

    await act(async () => {
      fireEvent.keyDown(window, { ctrlKey: true, shiftKey: true, key: 'B' });
    });

    expect(screen.queryByText(/too many attempts/i)).not.toBeInTheDocument();
    expect(screen.getByText(/sign-in lock cleared/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).not.toBeDisabled();
  });

  it('shows staff force-end notice when present', () => {
    loginNotice = 'Your session was ended by staff.';
    render(<LoginHomePage />);
    expect(screen.getByText(/ended by staff/i)).toBeInTheDocument();
  });

  it('auto-dismisses session expired message after delay', () => {
    vi.useFakeTimers();
    error = SESSION_EXPIRED_MESSAGE;
    render(<LoginHomePage />);
    expect(screen.getByText(SESSION_EXPIRED_MESSAGE)).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(SESSION_EXPIRED_DISMISS_MS);
    });

    expect(clearError).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it('does not auto-dismiss session expired when sign-in is locked', () => {
    vi.useFakeTimers();
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    error = SESSION_EXPIRED_MESSAGE;
    render(<LoginHomePage />);

    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    expect(screen.queryByText(SESSION_EXPIRED_MESSAGE)).not.toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(SESSION_EXPIRED_DISMISS_MS);
    });

    expect(clearError).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
