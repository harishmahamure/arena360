export interface BalanceUpdatedPayload {
  balanceId?: string;
  /** Raw wallet minutes from the server (ADR-0018). */
  remainingMinutes?: number;
  sessionId?: string;
  playerId?: string;
}

export interface SessionSnapshot {
  id: string;
  balanceId: string;
  walletBalanceMinutes: number;
}

/**
 * Apply a `balance.updated` realtime payload to the active session, if it matches.
 * Returns the updated session or null when the event should be ignored.
 */
export function applyBalanceUpdated<T extends SessionSnapshot>(
  activeSession: T | null,
  payload: BalanceUpdatedPayload | undefined,
): T | null {
  if (!activeSession || typeof payload?.remainingMinutes !== 'number') {
    return null;
  }

  const matchesSession =
    typeof payload.sessionId === 'string' && payload.sessionId === activeSession.id;
  const matchesBalance =
    typeof payload.balanceId === 'string' && payload.balanceId === activeSession.balanceId;

  if (!matchesSession && !matchesBalance) {
    return null;
  }

  return {
    ...activeSession,
    walletBalanceMinutes: payload.remainingMinutes,
  };
}

/**
 * Merge a poll/reconcile response with the current session.
 * Preserves the higher raw wallet minutes so a stale cache read cannot
 * regress a mid-session recharge applied via WebSocket.
 */
export function mergeReconciledSession<T extends SessionSnapshot>(polled: T, current: T | null): T {
  if (!current || current.id !== polled.id) {
    return polled;
  }
  return {
    ...polled,
    walletBalanceMinutes: Math.max(polled.walletBalanceMinutes, current.walletBalanceMinutes),
  };
}
