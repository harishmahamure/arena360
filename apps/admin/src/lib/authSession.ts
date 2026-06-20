import { local } from '@gaming-cafe/utils';
import type { Dispatch } from 'react';
import type { AuthAction } from '../store/auth/action';
import { loadState } from '../store/persistance';

export interface AuthExpiredContext {
  url?: string;
  message?: string;
  authHeader?: string;
}

const TOKEN_AUTH_MESSAGES = new Set(['Invalid or expired token', 'Authentication required']);

const CREDENTIAL_ENTRY_PATHS = ['/auth/login/admin', '/auth/login/staff'];

interface JwtPayload {
  userId?: string;
  roles?: string[];
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
  if (!isTokenAuthFailure(context.message)) return false;
  if (isStaleUnauthorized(context)) return false;
  return Boolean(local.get('accessToken'));
}

export function registerAdminAuthSession(handlers: { onSessionExpired: () => void }): void {
  sessionExpiredHandler = handlers.onSessionExpired;
}

export function clearAdminSession(): void {
  local.remove('accessToken');
}

export function decodeJwtPayload(token: string): JwtPayload | null {
  try {
    const [, payloadB64] = token.split('.');
    if (!payloadB64) return null;
    const json = atob(payloadB64.replace(/-/g, '+').replace(/_/g, '/'));
    return JSON.parse(json) as JwtPayload;
  } catch {
    return null;
  }
}

export function bootstrapAuthFromToken(dispatch: Dispatch<AuthAction>): void {
  const token = local.get<string>('accessToken');
  if (!token) return;

  const persisted = loadState();
  if (persisted?.auth?.role) return;

  const payload = decodeJwtPayload(token);
  if (!payload?.userId) return;

  const role = (payload.roles?.[0] ?? '').toLowerCase();
  if (role !== 'admin' && role !== 'staff') return;

  dispatch({
    type: 'SetAuthDetail',
    payload: {
      id: payload.userId,
      username: '',
      email: '',
      firstName: '',
      lastName: '',
      role,
      isActive: true,
    },
  });
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
