import { describe, expect, it } from 'vitest';
import { isTokenAuthFailure, resolveAuthExpiredAction } from './authSession';

describe('authSession', () => {
  it('detects token auth failures', () => {
    expect(isTokenAuthFailure('Invalid or expired token')).toBe(true);
    expect(isTokenAuthFailure('Invalid credentials')).toBe(false);
    expect(isTokenAuthFailure('Invalid OTP')).toBe(false);
  });

  it('ignores credential-entry 401s', () => {
    expect(
      resolveAuthExpiredAction(
        { url: '/auth/login/admin', message: 'Invalid or expired token' },
        { phase: 'register', hasPlayerToken: false },
      ),
    ).toBeNull();
  });

  it('full-resets when the device token is rejected on idle', () => {
    expect(
      resolveAuthExpiredAction(
        { url: '/kiosk/sessions/current', message: 'Invalid or expired token' },
        { phase: 'login', hasPlayerToken: false },
      ),
    ).toBe('full-reset');
  });

  it('does not reset during registration or initial load', () => {
    expect(
      resolveAuthExpiredAction(
        { url: '/realtime', message: 'Invalid or expired token' },
        { phase: 'register', hasPlayerToken: false },
      ),
    ).toBeNull();
    expect(
      resolveAuthExpiredAction(
        { url: '/realtime', message: 'Invalid or expired token' },
        { phase: 'loading', hasPlayerToken: false },
      ),
    ).toBeNull();
  });

  it('logs out the player when a session token expires', () => {
    expect(
      resolveAuthExpiredAction(
        { url: '/kiosk/sessions/current', message: 'Invalid or expired token' },
        { phase: 'session', hasPlayerToken: true },
      ),
    ).toBe('player-logout');
  });

  it('ignores missing player header when no player token is stored', () => {
    expect(
      resolveAuthExpiredAction(
        { url: '/kiosk/sessions', message: 'X-Player-Token required for player routes' },
        { phase: 'session', hasPlayerToken: false },
      ),
    ).toBeNull();
  });
});
