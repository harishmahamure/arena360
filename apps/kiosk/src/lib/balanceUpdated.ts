export interface BalanceUpdatedPayload {
  balanceId?: string;
  remainingMinutes?: number;
  sessionId?: string;
  playerId?: string;
}

export interface SessionSnapshot {
  id: string;
  balanceId: string;
  remainingMinutes: number;
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
    remainingMinutes: payload.remainingMinutes,
  };
}
