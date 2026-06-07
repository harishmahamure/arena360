import { describe, expect, it } from 'vitest';
import { applyBalanceUpdated } from './balanceUpdated';

const session = {
  id: 'session-1',
  balanceId: 'balance-1',
  remainingMinutes: 30,
};

describe('applyBalanceUpdated', () => {
  it('updates remaining minutes when sessionId matches', () => {
    const result = applyBalanceUpdated(session, {
      sessionId: 'session-1',
      remainingMinutes: 55,
    });
    expect(result).toEqual({ ...session, remainingMinutes: 55 });
  });

  it('updates remaining minutes when balanceId matches', () => {
    const result = applyBalanceUpdated(session, {
      balanceId: 'balance-1',
      remainingMinutes: 45,
    });
    expect(result).toEqual({ ...session, remainingMinutes: 45 });
  });

  it('ignores events for a different session and balance', () => {
    expect(
      applyBalanceUpdated(session, {
        sessionId: 'other-session',
        balanceId: 'other-balance',
        remainingMinutes: 60,
      }),
    ).toBeNull();
  });

  it('ignores events without remainingMinutes', () => {
    expect(applyBalanceUpdated(session, { sessionId: 'session-1' })).toBeNull();
  });

  it('ignores events when there is no active session', () => {
    expect(
      applyBalanceUpdated(null, {
        sessionId: 'session-1',
        remainingMinutes: 60,
      }),
    ).toBeNull();
  });
});
