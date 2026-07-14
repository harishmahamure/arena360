import { beforeEach, describe, expect, it, vi } from 'vitest';
import { appendKioskLog } from './bootDiagnostics';
import {
  enqueueEndIntent,
  hasPendingEndIntents,
  loadEndIntents,
  logEndIntentReplayFailure,
  removeEndIntent,
} from './offlineQueue';

vi.mock('./bootDiagnostics', () => ({
  appendKioskLog: vi.fn(),
}));

describe('offlineQueue', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('is empty initially', () => {
    expect(loadEndIntents()).toHaveLength(0);
    expect(hasPendingEndIntents()).toBe(false);
  });

  it('enqueues an end intent', () => {
    enqueueEndIntent('session-1', 'offline_reconcile');
    const intents = loadEndIntents();
    expect(intents).toHaveLength(1);
    expect(intents[0]).toMatchObject({ sessionId: 'session-1', reason: 'offline_reconcile' });
  });

  it('dedupes per session with the latest reason winning', () => {
    enqueueEndIntent('session-1', 'voluntary');
    enqueueEndIntent('session-1', 'offline_reconcile');
    const intents = loadEndIntents();
    expect(intents).toHaveLength(1);
    expect(intents[0]?.reason).toBe('offline_reconcile');
  });

  it('removes an intent by session id', () => {
    enqueueEndIntent('session-1', 'force');
    enqueueEndIntent('session-2', 'auto');
    removeEndIntent('session-1');
    const remaining = loadEndIntents();
    expect(remaining).toHaveLength(1);
    expect(remaining[0]?.sessionId).toBe('session-2');
  });

  it('logs replay failures', () => {
    logEndIntentReplayFailure('session-1', new Error('network down'));
    expect(appendKioskLog).toHaveBeenCalledWith(
      'warn',
      expect.stringContaining('[offlineQueue] replay failed sessionId=session-1'),
    );
  });
});
