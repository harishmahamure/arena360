import { tokenCache } from './http';
import { realtimeUrl } from './config';

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

export class ConsoleRealtimeClient {
  private ws: WebSocket | null = null;
  private subscriptions = new Set<string>();
  private globalHandlers = new Set<RealtimeHandler>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private disposed = false;

  connect(): void {
    this.disposed = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    const token = tokenCache.device;
    if (!token) return;

    try {
      this.ws = new WebSocket(realtimeUrl(), ['bearer', token]);
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
      }

      for (const handler of this.globalHandlers) {
        handler(frame);
      }
    };

    this.ws.onclose = () => {
      this.ws = null;
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

  onAny(handler: RealtimeHandler): () => void {
    this.globalHandlers.add(handler);
    return () => this.globalHandlers.delete(handler);
  }

  disconnect(): void {
    this.disposed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
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
    const delay = Math.min(BASE_DELAY * 2 ** this.reconnectAttempts, MAX_DELAY);
    this.reconnectAttempts += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }
}
