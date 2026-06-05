/** API base URL — override for production builds (see .env.example). */
export const API_BASE_URL = 'http://10.0.2.2:3000';

export function realtimeUrl(): string {
  const wsProtocol = API_BASE_URL.startsWith('https') ? 'wss' : 'ws';
  const host = API_BASE_URL.replace(/^https?:\/\//, '');
  return `${wsProtocol}://${host}/realtime`;
}
