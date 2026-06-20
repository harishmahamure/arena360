import { local } from '@gaming-cafe/utils';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { isTokenAuthFailure, shouldLogoutOnUnauthorized } from './authSession';

describe('admin authSession', () => {
  beforeEach(() => {
    local.set('accessToken', 'current-token');
  });

  afterEach(() => {
    local.remove('accessToken');
  });

  it('detects token auth failures', () => {
    expect(isTokenAuthFailure('Invalid or expired token')).toBe(true);
    expect(isTokenAuthFailure('Authentication required')).toBe(true);
    expect(isTokenAuthFailure('Invalid credentials')).toBe(false);
  });

  it('ignores credential-entry 401s', () => {
    expect(
      shouldLogoutOnUnauthorized({
        url: '/auth/login/admin',
        message: 'Invalid or expired token',
      }),
    ).toBe(false);
  });

  it('ignores non-token 401 messages', () => {
    expect(
      shouldLogoutOnUnauthorized({
        url: '/stats/dashboard',
        message: 'User is not active',
      }),
    ).toBe(false);
  });

  it('logs out on token auth failure for authenticated requests', () => {
    expect(
      shouldLogoutOnUnauthorized({
        url: '/stats/dashboard',
        message: 'Invalid or expired token',
        authHeader: 'Bearer current-token',
      }),
    ).toBe(true);
  });

  it('skips stale 401s when the stored token changed', () => {
    expect(
      shouldLogoutOnUnauthorized({
        url: '/stats/dashboard',
        message: 'Invalid or expired token',
        authHeader: 'Bearer old-token',
      }),
    ).toBe(false);
  });
});
