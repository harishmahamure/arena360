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

  it('logs out when a protected request rejects an inactive account', () => {
    expect(
      shouldLogoutOnUnauthorized({
        url: '/stats/dashboard',
        message: 'User is not active',
      }),
    ).toBe(true);
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

import { Permission } from '@gaming-cafe/contracts';
import { vi } from 'vitest';
import {
  bootstrapAuthFromToken,
  clearAdminSession,
  panelClaims,
  sessionPermissions,
  watchSessionExpiry,
} from './authSession';

const jwt = (claims: object) => `header.${btoa(JSON.stringify(claims))}.signature`;

describe('panel session lifecycle', () => {
  afterEach(() => {
    localStorage.clear();
    vi.useRealTimers();
  });
  it('rejects malformed, expired, and non-panel tokens', () => {
    expect(panelClaims('invalid')).toBeNull();
    expect(panelClaims(jwt({ userId: 'u', roles: ['admin'] }))).toBeNull();
    expect(panelClaims(jwt({ userId: 'u', roles: ['admin'], exp: 1 }))).toBeNull();
    expect(
      panelClaims(jwt({ userId: 'u', roles: ['player'], exp: Date.now() / 1000 + 60 })),
    ).toBeNull();
  });
  it('does not trust a persisted administrator role for a staff token', () => {
    local.set(
      'accessToken',
      jwt({
        userId: 'staff',
        roles: ['staff'],
        exp: Date.now() / 1000 + 60,
        permissions: ['settings:read'],
      }),
    );
    local.set('state', JSON.stringify({ auth: { id: 'staff', role: 'admin' } }));
    const dispatch = vi.fn();
    bootstrapAuthFromToken(dispatch);
    expect(dispatch).toHaveBeenCalledWith(
      expect.objectContaining({ payload: expect.objectContaining({ role: 'staff' }) }),
    );
    expect(sessionPermissions()).not.toContain(Permission.SessionsWrite);
    expect(sessionPermissions()).toContain(Permission.SettingsRead);
    expect(sessionPermissions()).not.toContain(Permission.SettingsWrite);
    expect(sessionPermissions()).not.toContain(Permission.DevicesWrite);
  });
  it('requires explicit organization grants, including for administrators', () => {
    const claims = {
      userId: 'admin',
      roles: ['admin'],
      permissions: [],
      exp: Date.now() / 1000 + 60,
    };
    expect(sessionPermissions(claims)).not.toContain(Permission.DevicesWrite);
    expect(sessionPermissions(claims)).not.toContain(Permission.RulesPublish);
  });
  it('expires proactively and cleans up its timer', async () => {
    vi.useFakeTimers();
    local.set('accessToken', jwt({ userId: 'u', roles: ['admin'], exp: Date.now() / 1000 + 2 }));
    const expire = vi.fn();
    const stop = watchSessionExpiry(expire);
    await vi.advanceTimersByTimeAsync(2001);
    expect(expire).toHaveBeenCalledOnce();
    stop();
    expect(vi.getTimerCount()).toBe(0);
  });
  it('clears persisted identity and acknowledgement on logout', () => {
    local.set('state', 'private-profile');
    local.set('accessToken', 'token');
    local.set('realtime_last_ack_id', 22);
    clearAdminSession();
    expect(local.get('state')).toBeNull();
    expect(local.get('accessToken')).toBeNull();
    expect(local.get('realtime_last_ack_id')).toBeNull();
  });
  it('expires an idle session, while activity in another tab extends it', async () => {
    vi.useFakeTimers();
    local.set('accessToken', jwt({ userId: 'u', roles: ['admin'], exp: Date.now() / 1000 + 60 }));
    const expire = vi.fn();
    const stop = watchSessionExpiry(expire, 2000);
    await vi.advanceTimersByTimeAsync(1500);
    local.set('arena:last-activity', Date.now());
    await vi.advanceTimersByTimeAsync(1000);
    expect(expire).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(expire).toHaveBeenCalledOnce();
    stop();
  });
});
