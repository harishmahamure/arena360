import { createHttpClient, type HttpClient } from '@gaming-cafe/utils';
import { API_BASE_URL } from './config';
import { clearPlayerToken, getTokens, setDeviceToken, setPlayerToken } from './tauriCommands';

let client: HttpClient | null = null;

export function getHttpClient(): HttpClient {
  if (!client) {
    client = createHttpClient({
      baseUrl: API_BASE_URL,
      getAuthToken: () => {
        // Sync read not available — tokens loaded into memory via KioskProvider
        return tokenCache.device ?? undefined;
      },
      getDeviceToken: () => tokenCache.player ?? undefined,
      deviceTokenHeader: 'X-Player-Token',
      onUnauthorized: () => {
        void clearPlayerToken();
        tokenCache.player = undefined;
      },
    });
  }
  return client;
}

/** In-memory token cache synced with Tauri secure file store */
export const tokenCache: { device?: string; player?: string } = {};

export async function loadTokensIntoCache(): Promise<void> {
  const store = await getTokens();
  tokenCache.device = store.deviceToken;
  tokenCache.player = store.playerToken;
}

export async function persistDeviceToken(token: string): Promise<void> {
  tokenCache.device = token;
  await setDeviceToken(token);
}

export async function persistPlayerToken(token: string): Promise<void> {
  tokenCache.player = token;
  await setPlayerToken(token);
}

export async function clearPlayerSession(): Promise<void> {
  tokenCache.player = undefined;
  await clearPlayerToken();
}
