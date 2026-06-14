import { describe, expect, it } from 'vitest';
import { KioskRealtimeClient } from './realtime';

describe('KioskRealtimeClient.resetSubscriptions', () => {
  it('allows replacing channels before connect', () => {
    const client = new KioskRealtimeClient();
    client.subscribe(['device:a', 'user:player-1']);
    expect(() => client.resetSubscriptions(['device:b'])).not.toThrow();
    expect(() => client.connect()).not.toThrow();
  });
});
