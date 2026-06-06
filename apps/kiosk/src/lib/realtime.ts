import { realtimeUrl } from './config';
import { tokenCache } from './http';

export interface ServerFrame {
  type: string;
  msg_id?: number;
  channel?: string;
  event_type?: string;
  payload?: Record<string, unknown>;
  ts?: string;
  code?: string;
  message?: string;
}

export type RealtimeHandler = (frame: ServerFrame) => void;

const MAX_DELAY = 5_000;
const BASE_DELAY = 1_000;

export class KioskRealtimeClient {
  private ws: WebSocket | null = null;
  private subscriptions = new Set<string>();
  private handlers = new Map<string, Set<RealtimeHandler>>();
  private globalHandlers = new Set<RealtimeHandler>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private disposed = false;
  private intentionalClose = false;

  connect(): void {
    // Re-arm: a prior disconnect() sets `disposed` to suppress auto-reconnect,
    // but an explicit connect() means the caller wants a live socket again.
    this.disposed = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    const token = tokenCache.device;
    if (!token) return;

    const openedThisAttempt = { value: false };

    try {
      this.ws = new WebSocket(realtimeUrl(), ['bearer', token]);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      openedThisAttempt.value = true;
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
      }

      for (const handler of this.globalHandlers) {
        handler(frame);
      }

      if (frame.event_type) {
        const set = this.handlers.get(frame.event_type);
        if (set) {
          for (const handler of set) {
            handler(frame);
          }
        }
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (this.intentionalClose) {
        this.intentionalClose = false;
        return;
      }
      if (!this.disposed) this.scheduleReconnect();
    };

    this.ws.onerror = () => {
      this.ws?.close();
    };
  }

  subscribe(channels: string[]): void {
    for (const ch of channels) this.subscriptions.add(ch);
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.send({ type: 'Subscribe', channels });
    }
  }

  unsubscribe(channels: string[]): void {
    for (const ch of channels) this.subscriptions.delete(ch);
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.send({ type: 'Unsubscribe', channels });
    }
  }

  on(eventType: string, handler: RealtimeHandler): () => void {
    let set = this.handlers.get(eventType);
    if (!set) {
      set = new Set();
      this.handlers.set(eventType, set);
    }
    set.add(handler);
    return () => set.delete(handler);
  }

  onAny(handler: RealtimeHandler): () => void {
    this.globalHandlers.add(handler);
    return () => this.globalHandlers.delete(handler);
  }

  disconnect(): void {
    this.disposed = true;
    this.reconnectAttempts = 0;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.intentionalClose = true;
      this.ws.close();
      this.ws = null;
    }
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
    const delay = Math.min(BASE_DELAY * 2 ** this.reconnectAttempts, MAX_DELAY);
    this.reconnectAttempts += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }
}
