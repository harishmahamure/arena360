import { type ClientRealtimeFrame, decodeServerFrame, encodeClientFrame } from '@gaming-cafe/proto';
import { local } from '@gaming-cafe/utils';
import { handleAuthExpired } from '../authSession';

export interface ServerFrame {
  type: string;
  msg_id?: number;
  channel?: string;
  event_type?: string;
  payload?: Record<string, unknown>;
  ts?: string;
  user_id?: string;
  roles?: string[];
  channels?: string[];
  code?: string;
  message?: string;
}

export type RealtimeEventHandler = (frame: ServerFrame) => void;

const LAST_ACK_KEY = 'realtime_last_ack_id';
const MAX_RECONNECT_DELAY = 30_000;
const BASE_DELAY = 1_000;

export class RealtimeClient {
  private ws: WebSocket | null = null;
  private url: string;
  private subscriptions = new Set<string>();
  private handlers = new Map<string, Set<RealtimeEventHandler>>();
  private globalHandlers = new Set<RealtimeEventHandler>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private disposed = false;
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private lastReceived = 0;
  private connectionTimer: ReturnType<typeof setTimeout> | undefined;
  private statusHandler: (status: 'connecting' | 'connected' | 'offline') => void;

  constructor(
    url: string,
    onStatus: (status: 'connecting' | 'connected' | 'offline') => void = () => {},
  ) {
    this.url = url;
    this.statusHandler = onStatus;
  }

  connect(): void {
    if (this.disposed || this.ws) return;

    const token = local.get<string>('accessToken');
    if (!token) return;

    this.statusHandler('connecting');
    try {
      this.ws = new WebSocket(this.url, ['arena360.protobuf.v1', 'bearer', token]);
      this.ws.binaryType = 'arraybuffer';
    } catch {
      this.scheduleReconnect();
      return;
    }

    const socket = this.ws;
    this.connectionTimer = setTimeout(() => socket.close(), 10_000);
    socket.onopen = () => {
      if (this.disposed || this.ws !== socket) return;
      clearTimeout(this.connectionTimer);
      this.lastReceived = Date.now();
      this.statusHandler('connected');
      this.heartbeat = setInterval(() => {
        if (Date.now() - this.lastReceived > 60_000) socket.close();
        else this.send({ type: 'Ping' });
      }, 25_000);
      this.reconnectAttempts = 0;
      if (this.subscriptions.size > 0) {
        this.send({ type: 'Subscribe', channels: [...this.subscriptions] });
      }
    };

    socket.onmessage = (event) => {
      if (this.disposed || this.ws !== socket) return;
      this.lastReceived = Date.now();
      let frame: ServerFrame;
      try {
        frame = decodeServerFrame(new Uint8Array(event.data as ArrayBuffer));
      } catch {
        return;
      }

      if (frame.type === 'Pong') return;

      let delivered = true;
      for (const handler of this.globalHandlers) {
        try {
          handler(frame);
        } catch {
          delivered = false;
        }
      }

      if (frame.event_type) {
        const eventHandlers = this.handlers.get(frame.event_type);
        if (eventHandlers) {
          for (const handler of eventHandlers) {
            try {
              handler(frame);
            } catch {
              delivered = false;
            }
          }
        }
      }
      if (delivered && frame.type === 'Event' && frame.msg_id != null) {
        this.send({ type: 'Ack', msg_id: frame.msg_id });
        local.set(LAST_ACK_KEY, frame.msg_id);
      }
    };

    socket.onclose = (event) => {
      if (this.ws !== socket) return;
      clearTimeout(this.connectionTimer);
      if (this.heartbeat) clearInterval(this.heartbeat);
      this.heartbeat = null;
      this.ws = null;
      this.statusHandler('offline');
      if (event?.code === 4001) {
        this.disposed = true;
        void handleAuthExpired({ url: '/realtime', authHeader: token });
        return;
      }
      if (!this.disposed) {
        this.scheduleReconnect();
      }
    };

    socket.onerror = () => {
      socket.close();
    };
  }

  subscribe(channels: string[]): void {
    for (const ch of channels) {
      this.subscriptions.add(ch);
    }
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.send({ type: 'Subscribe', channels });
    }
  }

  unsubscribe(channels: string[]): void {
    for (const ch of channels) {
      this.subscriptions.delete(ch);
    }
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.send({ type: 'Unsubscribe', channels });
    }
  }

  on(eventType: string, handler: RealtimeEventHandler): () => void {
    let set = this.handlers.get(eventType);
    if (!set) {
      set = new Set();
      this.handlers.set(eventType, set);
    }
    set.add(handler);
    return () => set.delete(handler);
  }

  onAny(handler: RealtimeEventHandler): () => void {
    this.globalHandlers.add(handler);
    return () => this.globalHandlers.delete(handler);
  }

  publish(channel: string, payload: Record<string, unknown>): void {
    this.send({ type: 'Publish', channel, payload });
  }

  disconnect(): void {
    this.disposed = true;
    clearTimeout(this.connectionTimer);
    if (this.heartbeat) clearInterval(this.heartbeat);
    this.heartbeat = null;
    this.statusHandler('offline');
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  private send(data: ClientRealtimeFrame): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(encodeClientFrame(data));
    }
  }

  private scheduleReconnect(): void {
    if (this.disposed || this.reconnectTimer) return;

    const delay = Math.min(BASE_DELAY * 2 ** this.reconnectAttempts, MAX_RECONNECT_DELAY);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }
}
