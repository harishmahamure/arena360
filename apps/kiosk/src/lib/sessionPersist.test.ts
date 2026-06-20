import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  clearPlayerPersistedState,
  readSessionSnapshot,
  readStoredPlayerName,
  storePlayerName,
  storeSessionSnapshot,
} from './sessionPersist';

const SESSION_SNAPSHOT_KEY = 'gaming-cafe.kiosk.session_snapshot';
const PLAYER_NAME_KEY = 'gaming-cafe.kiosk.player_name';

describe('sessionPersist', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('stores and reads player display name', () => {
    storePlayerName('player1');
    expect(readStoredPlayerName()).toBe('player1');
    clearPlayerPersistedState();
    expect(readStoredPlayerName()).toBeNull();
  });

  it('stores and reads session snapshot', () => {
    storeSessionSnapshot({
      id: 'session-1',
      startTime: '2026-06-20T12:00:00Z',
      balanceId: 'balance-1',
      walletBalanceMinutes: 60,
    });
    expect(readSessionSnapshot()).toMatchObject({
      id: 'session-1',
      walletBalanceMinutes: 60,
    });
  });

  it('rejects malformed session snapshot JSON', () => {
    localStorage.setItem(SESSION_SNAPSHOT_KEY, JSON.stringify({ id: 'only-id' }));
    expect(readSessionSnapshot()).toBeNull();
  });

  it('clearPlayerPersistedState removes name and snapshot', () => {
    storePlayerName('player1');
    storeSessionSnapshot({
      id: 'session-1',
      startTime: '2026-06-20T12:00:00Z',
      balanceId: 'balance-1',
      walletBalanceMinutes: 30,
    });
    clearPlayerPersistedState();
    expect(localStorage.getItem(PLAYER_NAME_KEY)).toBeNull();
    expect(localStorage.getItem(SESSION_SNAPSHOT_KEY)).toBeNull();
  });
});
