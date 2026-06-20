/**
 * Player session snapshot + display name for hard-refresh recovery (ADR-0017 OQ-9).
 * Tokens live in Tauri secure storage; this holds non-secret UI/session fields
 * so the kiosk can re-enter the session phase before the network returns.
 */

import type { DeductionProfile } from '@gaming-cafe/contracts';

const PLAYER_NAME_KEY = 'gaming-cafe.kiosk.player_name';
const SESSION_SNAPSHOT_KEY = 'gaming-cafe.kiosk.session_snapshot';

export interface SessionSnapshot {
  id: string;
  startTime: string;
  balanceId: string;
  walletBalanceMinutes: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string | null;
}

export function readStoredPlayerName(): string | null {
  try {
    return localStorage.getItem(PLAYER_NAME_KEY);
  } catch {
    return null;
  }
}

export function storePlayerName(name: string | null): void {
  try {
    if (name) localStorage.setItem(PLAYER_NAME_KEY, name);
    else localStorage.removeItem(PLAYER_NAME_KEY);
  } catch {
    // localStorage unavailable — non-fatal
  }
}

function isSessionSnapshot(value: unknown): value is SessionSnapshot {
  if (!value || typeof value !== 'object') return false;
  const s = value as SessionSnapshot;
  return (
    typeof s.id === 'string' &&
    typeof s.startTime === 'string' &&
    typeof s.balanceId === 'string' &&
    typeof s.walletBalanceMinutes === 'number'
  );
}

export function readSessionSnapshot(): SessionSnapshot | null {
  try {
    const raw = localStorage.getItem(SESSION_SNAPSHOT_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return isSessionSnapshot(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function storeSessionSnapshot(session: SessionSnapshot): void {
  try {
    localStorage.setItem(SESSION_SNAPSHOT_KEY, JSON.stringify(session));
  } catch {
    // non-fatal
  }
}

export function clearSessionSnapshot(): void {
  try {
    localStorage.removeItem(SESSION_SNAPSHOT_KEY);
  } catch {
    // non-fatal
  }
}

export function clearPlayerPersistedState(): void {
  storePlayerName(null);
  clearSessionSnapshot();
}
