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
      await vi.advanceTimersByTimeAsync(0);
      for (const delay of [1_000, 2_000, 4_000]) {
        await vi.advanceTimersByTimeAsync(delay);
      }
      const reconnectLogs = vi
        .mocked(appendKioskLog)
        .mock.calls.filter(
          ([level, msg]) => level === 'warn' && String(msg).includes('reconnect attempt'),
        );
      const attempts = reconnectLogs.map(([, message]) =>
        Number(String(message).match(/attempt (\d+)/)?.[1]),
      );
      expect(attempts[0]).toBe(1);
      expect(attempts.every((attempt) => attempt === 1 || attempt % 5 === 0)).toBe(true);
    } finally {
      globalThis.WebSocket = originalWebSocket;
      vi.useRealTimers();
    }
  });
});

describe('KioskRealtimeClient stale socket ownership', () => {
  it('does not null the replacement socket when an older socket closes', () => {
    const client = new KioskRealtimeClient();
    const sockets: WebSocket[] = [];

    const originalWebSocket = globalThis.WebSocket;
    class TrackingWebSocket {
      static readonly CONNECTING = 0;
      static readonly OPEN = 1;
      static readonly CLOSING = 2;
      static readonly CLOSED = 3;
      readyState = TrackingWebSocket.OPEN;
      onopen: (() => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;
      constructor() {
        sockets.push(this as unknown as WebSocket);
        queueMicrotask(() => this.onopen?.());
      }
      close() {
        this.readyState = TrackingWebSocket.CLOSED;
        queueMicrotask(() => this.onclose?.());
      }
      send() {}
    }
    globalThis.WebSocket = TrackingWebSocket as unknown as typeof WebSocket;

    try {
      client.connect();
      expect(sockets).toHaveLength(1);
      client.connect();
      expect(sockets).toHaveLength(2);
      sockets[0]?.close();
      expect(client.connected).toBe(true);
      client.disconnect();
    } finally {
      globalThis.WebSocket = originalWebSocket;
    }
  });
});
