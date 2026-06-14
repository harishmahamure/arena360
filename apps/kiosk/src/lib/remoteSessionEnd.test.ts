import { describe, expect, it } from 'vitest';
import {
  isRemoteSessionEndEvent,
  shouldEndSessionForRemoteEvent,
  shouldRunWsFailPoll,
  staffEndedFromPayload,
} from './remoteSessionEnd';

describe('isRemoteSessionEndEvent', () => {
  it('matches session.ended and session.force_logout', () => {
    expect(isRemoteSessionEndEvent('session.ended')).toBe(true);
    expect(isRemoteSessionEndEvent('session.force_logout')).toBe(true);
  });

  it('rejects unrelated events', () => {
    expect(isRemoteSessionEndEvent('balance.updated')).toBe(false);
  });
});

describe('shouldEndSessionForRemoteEvent', () => {
  it('accepts when session ids match', () => {
    expect(
      shouldEndSessionForRemoteEvent('session-1', { sessionId: 'session-1', reason: 'force' }),
    ).toBe(true);
  });

  it('accepts when payload omits sessionId', () => {
    expect(shouldEndSessionForRemoteEvent('session-1', { reason: 'force' })).toBe(true);
  });

  it('rejects mismatched sessionId', () => {
    expect(
      shouldEndSessionForRemoteEvent('session-1', { sessionId: 'other', reason: 'force' }),
    ).toBe(false);
  });
});

describe('staffEndedFromPayload', () => {
  it('detects force reason on session.ended', () => {
    expect(staffEndedFromPayload('session.ended', { reason: 'force' })).toBe(true);
    expect(staffEndedFromPayload('session.ended', { reason: 'voluntary' })).toBe(false);
  });

  it('treats session.force_logout as staff ended', () => {
    expect(staffEndedFromPayload('session.force_logout', {})).toBe(true);
  });
});

describe('shouldRunWsFailPoll', () => {
  it('polls only during session when WS is down and player token exists', () => {
    expect(shouldRunWsFailPoll('session', false, true)).toBe(true);
    expect(shouldRunWsFailPoll('session', true, true)).toBe(false);
    expect(shouldRunWsFailPoll('login', false, true)).toBe(false);
    expect(shouldRunWsFailPoll('session', false, false)).toBe(false);
  });
});
