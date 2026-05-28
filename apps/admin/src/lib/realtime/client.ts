import { local } from '@gaming-cafe/utils';

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

  constructor(url: string) {
    this.url = url;
  }

  connect(): void {
    if (this.disposed) return;

    const token = local.get<string>('accessToken');
    if (!token) return;

    try {
      this.ws = new WebSocket(this.url, ['bearer', token]);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      if (this.subscriptions.size > 0) {
        this.send({ type: 'Subscribe', channels: [...this.subscriptions] });
      }
    };

    this.ws.onmessage = (event) => {
      let frame: ServerFrame;
      try {
        frame = JSON.parse(event.data as string);
      } catch {
        return;
      }

      if (frame.type === 'Event' && frame.msg_id != null) {
        this.send({ type: 'Ack', msg_id: frame.msg_id });
        local.set(LAST_ACK_KEY, frame.msg_id);
      }

      if (frame.type === 'Pong') return;

      for (const handler of this.globalHandlers) {
        handler(frame);
      }

      if (frame.event_type) {
        const eventHandlers = this.handlers.get(frame.event_type);
        if (eventHandlers) {
          for (const handler of eventHandlers) {
            handler(frame);
          }
        }
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (!this.disposed) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = () => {
      this.ws?.close();
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

  private send(data: Record<string, unknown>): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
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
