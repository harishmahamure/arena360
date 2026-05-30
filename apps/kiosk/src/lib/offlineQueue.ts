/**
 * Offline end-intent queue (D18). When a session end cannot reach the backend
 * (network down at end time), we persist the intent locally and replay it on
 * reconnect with reason `offline_reconcile`. The backend end is idempotent:
 * ending an already-closed session is a no-op, so replays never double-deduct.
 */

const STORAGE_KEY = 'gaming-cafe.kiosk.end_intents';

export interface EndIntent {
  sessionId: string;
  reason: string;
  queuedAt: number;
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
  } catch {
    return [];
  }
}

function save(intents: EndIntent[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(intents));
  } catch {
    // non-fatal
  }
}

export function enqueueEndIntent(sessionId: string, reason: string): void {
  const intents = loadEndIntents();
  // Dedupe per session — the latest reason wins.
  const next = intents.filter((i) => i.sessionId !== sessionId);
  next.push({ sessionId, reason, queuedAt: Date.now() });
  save(next);
}

export function removeEndIntent(sessionId: string): void {
  save(loadEndIntents().filter((i) => i.sessionId !== sessionId));
}

export function hasPendingEndIntents(): boolean {
  return loadEndIntents().length > 0;
}
