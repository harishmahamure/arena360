const DEFAULT_API_URL = 'http://localhost:3000';

function apiUrlFromEnv(): string {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return DEFAULT_API_URL;
}

export const API_BASE_URL = apiUrlFromEnv();

function numberFromEnv(key: string, fallback: number): number {
  const raw =
    typeof import.meta !== 'undefined'
      ? (import.meta.env as Record<string, string | undefined>)?.[key]
      : undefined;
  const parsed = raw ? Number.parseInt(raw, 10) : Number.NaN;
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

/**
 * How long a session may keep counting down locally after the backend becomes
 * unreachable before the station re-locks (D17). Configurable via
 * `VITE_OFFLINE_GRACE_MINUTES`; defaults to 5 minutes.
 */
export const OFFLINE_GRACE_MS = numberFromEnv('VITE_OFFLINE_GRACE_MINUTES', 5) * 60_000;

export function realtimeUrl(): string {
  const apiUrl = API_BASE_URL;
  const wsProtocol = apiUrl.startsWith('https') ? 'wss' : 'ws';
  const host = apiUrl.replace(/^https?:\/\//, '');
  return `${wsProtocol}://${host}/realtime`;
}
