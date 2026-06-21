import { describe, expect, it } from 'vitest';
import { applyBalanceUpdated, mergeReconciledSession } from './balanceUpdated';

const session = {
  id: 'session-1',
  balanceId: 'balance-1',
  walletBalanceMinutes: 30,
};

describe('applyBalanceUpdated', () => {
  it('updates wallet balance when sessionId matches', () => {
    const result = applyBalanceUpdated(session, {
      sessionId: 'session-1',
      remainingMinutes: 55,
    });
    expect(result).toEqual({ ...session, walletBalanceMinutes: 55 });
  });

  it('updates wallet balance when balanceId matches', () => {
    const result = applyBalanceUpdated(session, {
      balanceId: 'balance-1',
      remainingMinutes: 45,
    });
    expect(result).toEqual({ ...session, walletBalanceMinutes: 45 });
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

describe('mergeReconciledSession', () => {
  it('keeps the higher wallet balance when poll is stale', () => {
    const polled = { ...session, walletBalanceMinutes: 30 };
    const current = { ...session, walletBalanceMinutes: 90 };
    expect(mergeReconciledSession(polled, current)).toEqual({
      ...polled,
      walletBalanceMinutes: 90,
    });
  });

  it('uses polled balance when it is higher', () => {
    const polled = { ...session, walletBalanceMinutes: 120 };
    const current = { ...session, walletBalanceMinutes: 90 };
    expect(mergeReconciledSession(polled, current)).toEqual(polled);
  });

  it('returns polled session when there is no current session', () => {
    const polled = { ...session, walletBalanceMinutes: 45 };
    expect(mergeReconciledSession(polled, null)).toEqual(polled);
  });
});
