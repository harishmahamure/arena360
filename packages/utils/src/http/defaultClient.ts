import { local } from '../lib/helpers';
import { createHttpClient } from './createHttpClient';

function defaultBaseUrl(): string {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return 'http://localhost:3001';
}

function defaultOnUnauthorized(): void {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    local.remove('accessToken');
    window.location.href = '/login';
  } catch (_error) {}
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
  onUnauthorized: defaultOnUnauthorized,
});
