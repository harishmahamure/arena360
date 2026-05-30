const DEFAULT_API_URL = 'http://localhost:3000';

function apiUrlFromEnv(): string {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) {
    return import.meta.env.VITE_API_URL;
  }
  return DEFAULT_API_URL;
}

export const API_BASE_URL = apiUrlFromEnv();

export function realtimeUrl(): string {
  const apiUrl = API_BASE_URL;
  const wsProtocol = apiUrl.startsWith('https') ? 'wss' : 'ws';
  const host = apiUrl.replace(/^https?:\/\//, '');
  return `${wsProtocol}://${host}/realtime`;
}
