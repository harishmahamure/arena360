import { local } from '../lib/helpers';
import { createHttpClient } from './createHttpClient';
import type { CreateHttpClientOptions } from './types';

function defaultBaseUrl(): string {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return 'http://localhost:3000';
}

type UnauthorizedContext =
  NonNullable<CreateHttpClientOptions['onUnauthorized']> extends (context: infer C) => void
    ? C
    : never;

let customOnUnauthorized: CreateHttpClientOptions['onUnauthorized'] | undefined;
let onMutationSuccess: CreateHttpClientOptions['onMutationSuccess'];

/** Replace the default 401 handler (e.g. admin selective session expiry). */
export function configureDefaultHttpClient(options: {
  onUnauthorized?: CreateHttpClientOptions['onUnauthorized'];
  onMutationSuccess?: CreateHttpClientOptions['onMutationSuccess'];
}): void {
  customOnUnauthorized = options.onUnauthorized;
  onMutationSuccess = options.onMutationSuccess;
}

function defaultOnUnauthorized(context?: UnauthorizedContext): void {
  if (typeof window === 'undefined') {
    return;
  }

  const requestUrl = context?.url ?? '';
  if (requestUrl.includes('/auth/login')) {
    // Failed login attempts return 401 — do not reload the login page.
    return;
  }

  try {
    local.remove('accessToken');

    if (window.location.pathname === '/login') {
      return;
    }

    window.location.href = '/login';
  } catch (_error) {}
}

function dispatchUnauthorized(context: UnauthorizedContext): void {
  if (customOnUnauthorized) {
    customOnUnauthorized(context);
    return;
  }
  defaultOnUnauthorized(context);
}

function defaultGetAuthToken(): string | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const token = local.get<string>('accessToken');
  return token ?? null;
}

export const http = createHttpClient({
  baseUrl: defaultBaseUrl(),
  getAuthToken: defaultGetAuthToken,
  onUnauthorized: dispatchUnauthorized,
  onMutationSuccess: (context) => onMutationSuccess?.(context),
});
