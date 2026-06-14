export interface SessionEndPayload {
  sessionId?: string;
  reason?: string;
  action?: string;
}

const REMOTE_SESSION_END_EVENTS = new Set(['session.ended', 'session.force_logout']);

export function isRemoteSessionEndEvent(eventType: string): boolean {
  return REMOTE_SESSION_END_EVENTS.has(eventType);
}

/** Ignore stale events for a different open session on this station. */
export function shouldEndSessionForRemoteEvent(
  activeSessionId: string | undefined,
  payload: SessionEndPayload | undefined,
): boolean {
  const remoteId = payload?.sessionId;
  if (remoteId && activeSessionId && remoteId !== activeSessionId) {
    return false;
  }
  return true;
}

export function staffEndedFromPayload(
  eventType: string,
  payload: SessionEndPayload | undefined,
): boolean {
  if (eventType === 'session.force_logout') return true;
  return payload?.reason === 'force';
}

/** Poll GET /kiosk/sessions/current only while WS is down during an active session. */
export function shouldRunWsFailPoll(
  phase: string,
  wsConnected: boolean,
  hasPlayerToken: boolean,
): boolean {
  return phase === 'session' && hasPlayerToken && !wsConnected;
}
