import { beforeEach, describe, expect, it, vi } from 'vitest';
import { appendKioskLog } from './bootDiagnostics';
import { KioskRealtimeClient } from './realtime';

vi.mock('./bootDiagnostics', () => ({
  appendKioskLog: vi.fn(),
}));

vi.mock('./config', () => ({
  realtimeUrl: () => 'ws://localhost/ws',
}));

vi.mock('./http', () => ({
  tokenCache: { device: 'device-token' },
}));

describe('KioskRealtimeClient.resetSubscriptions', () => {
  it('allows replacing channels before connect', () => {
    const client = new KioskRealtimeClient();
    client.subscribe(['device:a', 'user:player-1']);
    expect(() => client.resetSubscriptions(['device:b'])).not.toThrow();
    expect(() => client.connect()).not.toThrow();
  });
});

describe('KioskRealtimeClient failure logging', () => {
  beforeEach(() => {
    vi.mocked(appendKioskLog).mockReset();
  });

  it('logs malformed WebSocket frames', () => {
    const client = new KioskRealtimeClient();
    client.connect();

    const ws = (client as unknown as { ws: WebSocket }).ws;
    ws.onmessage?.({ data: 'not-json' } as MessageEvent);

    expect(appendKioskLog).toHaveBeenCalledWith('warn', '[realtime] malformed WebSocket frame');
  });

  it('rate-limits reconnect attempt logging', async () => {
    vi.useFakeTimers();
    const client = new KioskRealtimeClient();

    const originalWebSocket = globalThis.WebSocket;
    class FailingWebSocket {
      static readonly CONNECTING = 0;
      static readonly OPEN = 1;
      static readonly CLOSING = 2;
      static readonly CLOSED = 3;
      readyState = FailingWebSocket.CLOSED;
      onopen: (() => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;
      constructor() {
        setTimeout(() => this.onclose?.(), 0);
      }
      close() {}
      send() {}
    }
    globalThis.WebSocket = FailingWebSocket as unknown as typeof WebSocket;

    try {
      client.connect();
      await vi.runOnlyPendingTimersAsync();
      for (let i = 0; i < 3; i++) {
        await vi.runOnlyPendingTimersAsync();
      }
      const reconnectLogs = vi
        .mocked(appendKioskLog)
        .mock.calls.filter(
          ([level, msg]) => level === 'warn' && String(msg).includes('reconnect attempt'),
        );
      expect(reconnectLogs).toHaveLength(1);
      expect(reconnectLogs[0]?.[1]).toContain('reconnect attempt 1');
    } finally {
      globalThis.WebSocket = originalWebSocket;
      vi.useRealTimers();
    }
  });
});
