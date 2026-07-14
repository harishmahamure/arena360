import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SESSION_EXPIRED_DISMISS_MS, SESSION_EXPIRED_MESSAGE } from '../lib/authMessages';
import { MAX_FAILURES } from '../lib/loginLockout';
import { LoginHomePage } from './LoginHomePage';

const playerLogin = vi.fn();
const clearLoginNotice = vi.fn();
const clearError = vi.fn();
const staffClearLoginLockout = vi.fn();
const enterSetup = vi.fn();

const { restartStation, shutdownStation, sleepStation } = vi.hoisted(() => ({
  restartStation: vi.fn(),
  shutdownStation: vi.fn(),
  sleepStation: vi.fn(),
}));

let loginNotice: string | null = null;
let error: string | null = null;
let staffLockoutClearTick = 0;

vi.mock('../context/KioskProvider', () => ({
  useKiosk: () => ({
    playerLogin,
    goToCreateAccount: vi.fn(),
    enterSetup,
    staffClearLoginLockout,
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
    get staffLockoutClearTick() {
      return staffLockoutClearTick;
    },
  }),
}));

vi.mock('../lib/tauriCommands', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/tauriCommands')>();
  return {
    ...actual,
    cachedAssetSrc: vi.fn(async (url: string) => url),
    sleepStation,
    restartStation,
    shutdownStation,
  };
});

describe('LoginHomePage', () => {
  beforeEach(() => {
    localStorage.clear();
    loginNotice = null;
    error = null;
    staffLockoutClearTick = 0;
    vi.clearAllMocks();
    vi.useRealTimers();
    staffClearLoginLockout.mockResolvedValue(undefined);
  });

  it('renders the sign-in form and station controls', () => {
    render(<LoginHomePage />);
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/app version/i)).toHaveTextContent(/^v\d/);
    expect(screen.getByRole('button', { name: /^sleep$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^restart$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^shutdown$/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/station controls/i)).toBeInTheDocument();
  });

  it('requires confirmation before restart', async () => {
    render(<LoginHomePage />);
    fireEvent.click(screen.getByRole('button', { name: /^restart$/i }));
    expect(screen.getByText(/restart this pc now/i)).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /yes, restart/i }));
    });
    expect(restartStation).toHaveBeenCalledTimes(1);
  });

  it('locks the form after too many failures', () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<LoginHomePage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: /clear sign-in lock \(staff\)/i }),
    ).toBeInTheDocument();
  });

  it('requires staff authentication to clear sign-in lock', async () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    render(<LoginHomePage />);

    fireEvent.click(screen.getByRole('button', { name: /clear sign-in lock \(staff\)/i }));
    expect(screen.getByRole('dialog', { name: /staff sign-in required/i })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/admin username/i), { target: { value: 'admin' } });
    fireEvent.change(screen.getByRole('dialog').querySelector('#kiosk-staff-password')!, {
      target: { value: 'secret' },
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /clear lock/i }));
    });

    expect(staffClearLoginLockout).toHaveBeenCalledWith('admin', 'secret', undefined);
  });

  it('staff lockout clear tick shows cleared banner and enables sign-in', async () => {
    const failures = Array.from({ length: MAX_FAILURES }, () => Date.now());
    localStorage.setItem('gaming-cafe.kiosk.login_failures', JSON.stringify(failures));
    const { rerender } = render(<LoginHomePage />);
    expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();

    localStorage.removeItem('gaming-cafe.kiosk.login_failures');
    staffLockoutClearTick = 1;
    await act(async () => {
      rerender(<LoginHomePage />);
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

  it('normalizes username and trims password on submit', async () => {
    playerLogin.mockResolvedValueOnce(undefined);
    render(<LoginHomePage />);

    fireEvent.change(screen.getByLabelText(/^username$/i), {
      target: { value: ' Yuvraj ' },
    });
    fireEvent.change(screen.getByLabelText(/^password$/i), {
      target: { value: '  secret  ' },
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /sign in/i }));
    });

    expect(playerLogin).toHaveBeenCalledWith('Yuvraj', 'secret');
  });
});
