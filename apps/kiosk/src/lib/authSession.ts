export interface AuthExpiredContext {
  url?: string;
  message?: string;
}

export type AuthExpiredAction = 'player-logout' | 'full-reset';

const TOKEN_AUTH_MESSAGES = new Set([
  'Invalid or expired token',
  'Authentication required',
  'Device authentication required',
  'X-Player-Token required for player routes',
  'Player token missing deviceId',
  'Invalid player ID in token',
]);

/** Endpoints where 401 means bad user input, not a stale stored token. */
const CREDENTIAL_ENTRY_PATHS = ['/auth/login/admin', '/auth/login/staff'];

const PLAYER_TOKEN_MESSAGES = new Set([
  'X-Player-Token required for player routes',
  'Player token missing deviceId',
  'Invalid player ID in token',
]);

export function isTokenAuthFailure(message: string | undefined): boolean {
  if (!message) return false;
  return TOKEN_AUTH_MESSAGES.has(message);
}

function isCredentialEntryPath(url: string | undefined): boolean {
  if (!url) return false;
  return CREDENTIAL_ENTRY_PATHS.some((path) => url.includes(path));
}

export interface AuthSessionRuntime {
  phase: string;
  hasPlayerToken: boolean;
}

export function resolveAuthExpiredAction(
  context: AuthExpiredContext,
  runtime: AuthSessionRuntime,
): AuthExpiredAction | null {
  const { url, message } = context;
  if (!isTokenAuthFailure(message)) return null;
  if (isCredentialEntryPath(url)) return null;

  if (message && PLAYER_TOKEN_MESSAGES.has(message)) {
    return 'player-logout';
  }

  if (
    runtime.hasPlayerToken &&
    (runtime.phase === 'session' || runtime.phase === 'already-in-session')
  ) {
    return 'player-logout';
  }

  // Only reset to registration when the station was previously provisioned.
  if (runtime.phase === 'register' || runtime.phase === 'loading') {
    return null;
  }

  return 'full-reset';
}

type PlayerLogoutHandler = () => void | Promise<void>;
type FullResetHandler = () => void | Promise<void>;

let playerLogoutHandler: PlayerLogoutHandler | null = null;
let fullResetHandler: FullResetHandler | null = null;
let getRuntime: (() => AuthSessionRuntime) | null = null;
let handling = false;

export function registerAuthSessionHandlers(options: {
  getRuntime: () => AuthSessionRuntime;
  onPlayerLogout: PlayerLogoutHandler;
  onFullReset: FullResetHandler;
}): void {
  getRuntime = options.getRuntime;
  playerLogoutHandler = options.onPlayerLogout;
  fullResetHandler = options.onFullReset;
}

export async function handleAuthExpired(context: AuthExpiredContext): Promise<void> {
  if (handling || !getRuntime) return;

  const action = resolveAuthExpiredAction(context, getRuntime());
  if (!action) return;

  handling = true;
  try {
    if (action === 'player-logout') {
      await playerLogoutHandler?.();
    } else {
      await fullResetHandler?.();
    }
  } finally {
    handling = false;
  }
}
