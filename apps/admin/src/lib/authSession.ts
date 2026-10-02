import { Permission } from '@gaming-cafe/contracts';
import { local } from '@gaming-cafe/utils';
import type { Dispatch } from 'react';
import type { AuthAction } from '../store/auth/action';
import { authInitialState } from '../store/auth/action';
import { loadState } from '../store/persistance';

export interface AuthExpiredContext {
  url?: string;
  message?: string;
  authHeader?: string;
}

const TOKEN_AUTH_MESSAGES = new Set(['Invalid or expired token', 'Authentication required']);

const CREDENTIAL_ENTRY_PATHS = ['/auth/login/admin', '/auth/login/staff', '/auth/login/panel'];

export interface JwtPayload {
  userId?: string;
  roles?: string[];
  permissions?: string[];
  tenantId?: string;
  orgIds?: string[];
  exp?: number;
}

let sessionExpiredHandler: (() => void) | null = null;
let handling = false;

export function isTokenAuthFailure(message: string | undefined): boolean {
  if (!message) return false;
  return TOKEN_AUTH_MESSAGES.has(message);
}

function isCredentialEntryPath(url: string | undefined): boolean {
  if (!url) return false;
  return CREDENTIAL_ENTRY_PATHS.some((path) => url.includes(path));
}

function normalizeBearer(value: string | undefined | null): string | null {
  if (!value) return null;
  const token = value.startsWith('Bearer ') ? value.slice(7) : value;
  return token.replaceAll('"', '').trim() || null;
}

function isStaleUnauthorized(context: AuthExpiredContext): boolean {
  const requestToken = normalizeBearer(context.authHeader);
  const currentToken = normalizeBearer(local.get<string>('accessToken'));
  if (!requestToken || !currentToken) return false;
  return requestToken !== currentToken;
}

export function shouldLogoutOnUnauthorized(context: AuthExpiredContext): boolean {
  if (isCredentialEntryPath(context.url)) return false;
  if (isStaleUnauthorized(context)) return false;
  return Boolean(local.get('accessToken'));
}

export function registerAdminAuthSession(handlers: { onSessionExpired: () => void }): () => void {
  sessionExpiredHandler = handlers.onSessionExpired;
  return () => {
    if (sessionExpiredHandler === handlers.onSessionExpired) sessionExpiredHandler = null;
  };
}

export function clearAdminSession(): void {
  local.remove('accessToken');
  local.remove('state');
  local.remove('realtime_last_ack_id');
  local.remove('arena:last-activity');
  window.dispatchEvent(new Event('arena:session-change'));
}

export function setAdminToken(token: string): void {
  local.set('arena:last-activity', Date.now());
  local.set('accessToken', token);
  window.dispatchEvent(new Event('arena:session-change'));
}

export function decodeJwtPayload(token: string): JwtPayload | null {
  try {
    const [, payloadB64] = token.split('.');
    if (!payloadB64) return null;
    const json = atob(payloadB64.replace(/-/g, '+').replace(/_/g, '/'));
    const value: unknown = JSON.parse(json);
    return value && typeof value === 'object' && !Array.isArray(value)
      ? (value as JwtPayload)
      : null;
  } catch {
    return null;
  }
}

/** Client claims guide the UI; every request is still authorized by the backend. */
export function panelClaims(token = local.get<string>('accessToken')): JwtPayload | null {
  if (typeof token !== 'string') return null;
  const claims = decodeJwtPayload(token);
  if (
    !claims ||
    typeof claims.userId !== 'string' ||
    !claims.userId ||
    !Array.isArray(claims.roles) ||
    !claims.roles.some((role) => role === 'admin' || role === 'staff') ||
    typeof claims.exp !== 'number' ||
    !Number.isFinite(claims.exp) ||
    claims.exp * 1000 <= Date.now()
  )
    return null;
  return claims;
}

export function sessionPermissions(claims = panelClaims()): Permission[] {
  if (!claims) return [];
  return (claims.permissions ?? []).filter((permission): permission is Permission =>
    Object.values(Permission).includes(permission as Permission),
  );
}

export function bootstrapAuthFromToken(dispatch: Dispatch<AuthAction>): void {
  const payload = panelClaims();
  if (!payload) {
    if (local.get('accessToken')) clearAdminSession();
    dispatch({ type: 'Reset' });
    return;
  }
  const persisted = loadState()?.auth;
  dispatch({
    type: 'SetAuthDetail',
    payload: {
      ...authInitialState,
      ...(persisted?.id === payload.userId ? persisted : {}),
      id: payload.userId!,
      role: payload.roles?.includes('admin') ? 'admin' : 'staff',
      isActive: true,
    },
  });
}

/** Expiry and inactivity checks also run after sleep and when returning to the tab. */
/** Renew this long before expiry so an active operator never hits an expired token mid-task. */
const RENEW_BEFORE_MS = 2 * 60_000;

export function watchSessionExpiry(
  onExpire: () => void,
  idleMs = idleTimeoutMs(),
  renew?: () => Promise<string>,
): () => void {
  let timer: ReturnType<typeof setTimeout>;
  let renewing = false;
  let stopped = false;
  let lastRenewAttempt = 0;
  const tryRenew = () => {
    if (!renew || renewing || Date.now() - lastRenewAttempt < 15_000) return;
    renewing = true;
    lastRenewAttempt = Date.now();
    renew()
      .then((token) => {
        if (!stopped && panelClaims(token)) local.set('accessToken', token);
      })
      .catch(() => {
        /* A failed renewal falls through to normal expiry handling. */
      })
      .finally(() => {
        renewing = false;
        if (!stopped) check();
      });
  };
  let lastActivity = local.get<number>('arena:last-activity') || Date.now();
  let lastSaved = lastActivity;
  const readActivity = () => {
    const saved = local.get<number>('arena:last-activity');
    if (typeof saved === 'number' && Number.isFinite(saved))
      lastActivity = Math.min(Date.now(), Math.max(lastActivity, saved));
    return lastActivity;
  };
  const check = () => {
    clearTimeout(timer);
    if (!local.get('accessToken')) return;
    const claims = panelClaims();
    const idleRemaining = idleMs - (Date.now() - readActivity());
    if (!claims || idleRemaining <= 0) {
      onExpire();
      return;
    }
    const expiresIn = (claims.exp ?? 0) * 1000 - Date.now();
    if (expiresIn <= RENEW_BEFORE_MS) tryRenew();
    timer = setTimeout(check, Math.max(0, Math.min(expiresIn, idleRemaining, 15_000)));
  };
  const activity = () => {
    if (Date.now() - readActivity() >= idleMs) {
      check();
      return;
    }
    lastActivity = Date.now();
    if (lastActivity - lastSaved >= Math.min(15_000, idleMs / 4)) {
      local.set('arena:last-activity', lastActivity);
      lastSaved = lastActivity;
    }
  };
  const events = ['pointerdown', 'keydown', 'scroll', 'touchstart'] as const;
  check();
  events.forEach((event) => {
    window.addEventListener(event, activity, { passive: true });
  });
  window.addEventListener('focus', check);
  document.addEventListener('visibilitychange', check);
  return () => {
    stopped = true;
    clearTimeout(timer);
    events.forEach((event) => {
      window.removeEventListener(event, activity);
    });
    window.removeEventListener('focus', check);
    document.removeEventListener('visibilitychange', check);
  };
}

function idleTimeoutMs(): number {
  const minutes = Number(import.meta.env.VITE_SESSION_IDLE_MINUTES ?? 30);
  return (Number.isFinite(minutes) && minutes >= 1 && minutes <= 1440 ? minutes : 30) * 60_000;
}

export async function handleAuthExpired(context: AuthExpiredContext): Promise<void> {
  if (handling || !shouldLogoutOnUnauthorized(context)) return;

  handling = true;
  try {
    clearAdminSession();
    sessionExpiredHandler?.();
  } finally {
    handling = false;
  }
}
