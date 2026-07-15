import { BRAND_LOGO_URL } from '@gaming-cafe/theme';
import pkg from '../../package.json';

/** Shipped kiosk app version (kept in sync with `src-tauri/tauri.conf.json`). */
export const KIOSK_APP_VERSION = pkg.version;

const DEFAULT_API_URL = 'http://localhost:3000';

function apiUrlFromEnv(): string {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return DEFAULT_API_URL;
}

export const API_BASE_URL = apiUrlFromEnv();

function stringFromEnv(key: string): string | null {
  const raw =
    typeof import.meta !== 'undefined'
      ? (import.meta.env as Record<string, string | undefined>)?.[key]
      : undefined;
  return raw?.trim() ? raw.trim() : null;
}

/** Transparent brand logo; override per venue via `VITE_KIOSK_LOGO_URL`. */
export const KIOSK_LOGO_URL = stringFromEnv('VITE_KIOSK_LOGO_URL') ?? BRAND_LOGO_URL;

/** Cinematic background loop on the login home; override per venue via `VITE_LOGIN_BACKGROUND_VIDEO_URL`. */
export const LOGIN_BACKGROUND_VIDEO_URL =
  stringFromEnv('VITE_LOGIN_BACKGROUND_VIDEO_URL') ?? 'https://cdn.arena360.cloud/launch.webm';

/** Bundled offline fallback when CDN/cache is unavailable in production builds. */
export const BUNDLED_LOGIN_BACKGROUND_VIDEO = '/launch.webm';

/** Centrally hosted media gallery for Setup picker; updated manually on CDN. */
export const GALLERY_URL =
  stringFromEnv('VITE_GALLERY_URL') ?? 'https://cdn.arena360.cloud/kiosk/gallery.json';

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

/**
 * When WebSocket is disconnected during an active session, poll
 * `GET /kiosk/sessions/current` on this interval as a fallback for remote ends.
 * Configurable via `VITE_SESSION_RECONCILE_SECONDS`; defaults to 30 seconds.
 * Legacy: `VITE_SESSION_RECONCILE_MINUTES` is still honored when seconds is unset.
 */
export const SESSION_RECONCILE_MS = (() => {
  const secondsRaw =
    typeof import.meta !== 'undefined'
      ? (import.meta.env as Record<string, string | undefined>)?.VITE_SESSION_RECONCILE_SECONDS
      : undefined;
  if (secondsRaw?.trim()) {
    const parsed = Number.parseInt(secondsRaw, 10);
    if (Number.isFinite(parsed) && parsed > 0) return parsed * 1000;
  }
  const minutes = numberFromEnv('VITE_SESSION_RECONCILE_MINUTES', 0);
  if (minutes > 0) return minutes * 60_000;
  return 30_000;
})();

/** Periodic reconcile during an active session (catches zombie WS). Default 45s. */
export const SESSION_HEALTH_RECONCILE_MS =
  numberFromEnv('VITE_SESSION_HEALTH_RECONCILE_SECONDS', 45) * 1000;

export function realtimeUrl(): string {
  const apiUrl = API_BASE_URL;
  const wsProtocol = apiUrl.startsWith('https') ? 'wss' : 'ws';
  const host = apiUrl.replace(/^https?:\/\//, '');
  return `${wsProtocol}://${host}/realtime`;
}
