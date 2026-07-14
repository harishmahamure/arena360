/**
 * Offline end-intent queue (D18). When a session end cannot reach the backend
 * (network down at end time), we persist the intent locally and replay it on
 * reconnect with reason `offline_reconcile`. The backend end is idempotent:
 * ending an already-closed session is a no-op, so replays never double-deduct.
 */

import { appendKioskLog } from './bootDiagnostics';

const STORAGE_KEY = 'gaming-cafe.kiosk.end_intents';

export interface EndIntent {
  sessionId: string;
  reason: string;
  queuedAt: number;
  /** Player who queued the intent — replay only for the same player. */
  playerId?: string;
}

export function loadEndIntents(): EndIntent[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.filter(
      (e): e is EndIntent =>
        typeof e?.sessionId === 'string' &&
        typeof e?.reason === 'string' &&
        typeof e?.queuedAt === 'number',
    );
  } catch (e) {
    void appendKioskLog('warn', `[offlineQueue] load failed: ${String(e)}`);
    return [];
  }
}

function save(intents: EndIntent[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(intents));
  } catch (e) {
    void appendKioskLog('warn', `[offlineQueue] save failed: ${String(e)}`);
  }
}

export function enqueueEndIntent(sessionId: string, reason: string, playerId?: string): void {
  const intents = loadEndIntents();
  const next = intents.filter((i) => i.sessionId !== sessionId);
  next.push({ sessionId, reason, queuedAt: Date.now(), playerId });
  save(next);
}

export function removeEndIntent(sessionId: string): void {
  save(loadEndIntents().filter((i) => i.sessionId !== sessionId));
}

export function clearEndIntents(): void {
  save([]);
}

export function hasPendingEndIntents(): boolean {
  return loadEndIntents().length > 0;
}

/** Intents owned by the current player (or legacy intents without playerId). */
export function endIntentsForPlayer(playerId: string | null): EndIntent[] {
  const intents = loadEndIntents();
  if (!playerId) return intents.filter((i) => !i.playerId);
  return intents.filter((i) => !i.playerId || i.playerId === playerId);
}

/** Log a failed replay attempt for a queued end intent. */
export function logEndIntentReplayFailure(sessionId: string, err: unknown): void {
  void appendKioskLog(
    'warn',
    `[offlineQueue] replay failed sessionId=${sessionId}: ${String(err)}`,
  );
}
