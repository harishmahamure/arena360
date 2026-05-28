import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RealtimeClient, type ServerFrame } from '../client';

class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState = MockWebSocket.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  sentMessages: string[] = [];
  protocol: string;

  constructor(
    public url: string,
    public protocols?: string | string[],
  ) {
    this.protocol = Array.isArray(protocols) ? protocols[0] : (protocols ?? '');
    setTimeout(() => {
      this.readyState = MockWebSocket.OPEN;
      this.onopen?.();
    }, 0);
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.();
  }

  simulateMessage(frame: ServerFrame) {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
}

let mockWsInstances: MockWebSocket[] = [];

beforeEach(() => {
  mockWsInstances = [];
  vi.stubGlobal(
    'WebSocket',
    class extends MockWebSocket {
      constructor(url: string, protocols?: string | string[]) {
        super(url, protocols);
        mockWsInstances.push(this);
      }
    },
  );

  localStorage.setItem('accessToken', '"test-jwt-token"');
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
  vi.useRealTimers();
});

describe('RealtimeClient', () => {
  it('connects with bearer protocol', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];
    expect(ws.url).toBe('ws://localhost:3000/realtime');
    expect(ws.protocols).toContain('bearer');

    client.disconnect();
  });

  it('does not connect without accessToken', () => {
    localStorage.removeItem('accessToken');
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();
    expect(mockWsInstances.length).toBe(0);
    client.disconnect();
  });

  it('subscribes to channels on connect', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.subscribe(['admin', 'staff']);
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];

    await vi.waitFor(() => expect(ws.sentMessages.length).toBeGreaterThan(0));

    const subscribeMsg = JSON.parse(ws.sentMessages[0]);
    expect(subscribeMsg.type).toBe('Subscribe');
    expect(subscribeMsg.channels).toEqual(['admin', 'staff']);

    client.disconnect();
  });

  it('sends ACK on Event frame', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];
    await vi.waitFor(() => expect(ws.readyState).toBe(MockWebSocket.OPEN));

    ws.simulateMessage({
      type: 'Event',
      msg_id: 42,
      channel: 'admin',
      event_type: 'test.event',
      payload: {},
      ts: new Date().toISOString(),
    });

    const ackMsg = ws.sentMessages.find((m) => JSON.parse(m).type === 'Ack');
    expect(ackMsg).toBeDefined();
    expect(JSON.parse(ackMsg ?? '').msg_id).toBe(42);

    client.disconnect();
  });

  it('dispatches event to specific handler', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    const handler = vi.fn();
    client.on('transaction.sale_completed', handler);
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];

    ws.simulateMessage({
      type: 'Event',
      msg_id: 1,
      channel: 'admin',
      event_type: 'transaction.sale_completed',
      payload: { amount: 100 },
      ts: new Date().toISOString(),
    });

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler.mock.calls[0][0].payload).toEqual({ amount: 100 });

    client.disconnect();
  });

  it('dispatches to global handler', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    const handler = vi.fn();
    client.onAny(handler);
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];

    ws.simulateMessage({
      type: 'Welcome',
      user_id: '00000000-0000-0000-0000-000000000001',
      roles: ['admin'],
    });

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler.mock.calls[0][0].type).toBe('Welcome');

    client.disconnect();
  });

  it('unsubscribe removes handler', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    const handler = vi.fn();
    const unsub = client.on('test', handler);

    client.connect();
    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];

    unsub();

    ws.simulateMessage({
      type: 'Event',
      msg_id: 1,
      channel: 'public',
      event_type: 'test',
      payload: {},
      ts: new Date().toISOString(),
    });

    expect(handler).not.toHaveBeenCalled();

    client.disconnect();
  });

  it('reconnects with exponential backoff', async () => {
    vi.useFakeTimers();
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.runAllTimersAsync();
    expect(mockWsInstances.length).toBe(1);

    const ws = mockWsInstances[0];
    ws.close();

    await vi.advanceTimersByTimeAsync(1000);
    expect(mockWsInstances.length).toBe(2);

    const ws2 = mockWsInstances[1];
    ws2.close();

    await vi.advanceTimersByTimeAsync(2000);
    expect(mockWsInstances.length).toBe(3);

    client.disconnect();
  });

  it('does not reconnect after disconnect', async () => {
    vi.useFakeTimers();
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.runAllTimersAsync();
    expect(mockWsInstances.length).toBe(1);

    client.disconnect();

    await vi.advanceTimersByTimeAsync(60000);
    expect(mockWsInstances.length).toBe(1);
  });

  it('persists last_ack_id to localStorage', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];

    ws.simulateMessage({
      type: 'Event',
      msg_id: 99,
      channel: 'admin',
      event_type: 'test',
      payload: {},
      ts: new Date().toISOString(),
    });

    const stored = localStorage.getItem('realtime_last_ack_id');
    expect(stored).toBeDefined();

    client.disconnect();
  });

  it('publish sends Publish frame', async () => {
    const client = new RealtimeClient('ws://localhost:3000/realtime');
    client.connect();

    await vi.waitFor(() => expect(mockWsInstances.length).toBe(1));
    const ws = mockWsInstances[0];
    await vi.waitFor(() => expect(ws.readyState).toBe(MockWebSocket.OPEN));

    client.publish('room:support-1', { text: 'hello' });

    const pubMsg = ws.sentMessages.find((m) => JSON.parse(m).type === 'Publish');
    expect(pubMsg).toBeDefined();
    const parsed = JSON.parse(pubMsg ?? '');
    expect(parsed.channel).toBe('room:support-1');
    expect(parsed.payload.text).toBe('hello');

    client.disconnect();
  });
});
