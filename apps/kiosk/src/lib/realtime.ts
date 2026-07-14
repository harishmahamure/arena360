import { appendKioskLog } from './bootDiagnostics';
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
/** Log reconnect attempts on the 1st and every 5th to avoid spam. */
const RECONNECT_LOG_EVERY = 5;

export class KioskRealtimeClient {
  private ws: WebSocket | null = null;
  private connectionGen = 0;
  private subscriptions = new Set<string>();
  private handlers = new Map<string, Set<RealtimeHandler>>();
  private globalHandlers = new Set<RealtimeHandler>();
  private connectHandlers = new Set<() => void>();
  private disconnectHandlers = new Set<() => void>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private disposed = false;
  private intentionalClose = false;

  connect(): void {
    this.disposed = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    const token = tokenCache.device;
    if (!token) return;

    if (this.ws) {
      this.intentionalClose = true;
      this.ws.close();
      this.ws = null;
      this.intentionalClose = false;
    }

    const gen = ++this.connectionGen;
    let socket: WebSocket;
    try {
      socket = new WebSocket(realtimeUrl(), ['bearer', token]);
    } catch (e) {
      void appendKioskLog('warn', `[realtime] WebSocket construct failed: ${String(e)}`);
      this.scheduleReconnect();
      return;
    }
    this.ws = socket;

    socket.onopen = () => {
      if (gen !== this.connectionGen || this.ws !== socket) return;
      this.reconnectAttempts = 0;
      if (this.subscriptions.size > 0) {
        this.sendOn(socket, { type: 'Subscribe', channels: [...this.subscriptions] });
      }
      for (const handler of this.connectHandlers) {
        handler();
      }
    };

    socket.onmessage = (event) => {
      if (gen !== this.connectionGen || this.ws !== socket) return;
      let frame: ServerFrame;
      try {
        frame = JSON.parse(event.data as string);
      } catch {
        void appendKioskLog('warn', '[realtime] malformed WebSocket frame');
        return;
      }

      if (frame.type === 'Event' && frame.msg_id != null) {
        this.sendOn(socket, { type: 'Ack', msg_id: frame.msg_id });
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

    socket.onclose = () => {
      if (gen !== this.connectionGen) return;
      if (this.ws === socket) {
        this.ws = null;
      }
      if (this.intentionalClose) {
        this.intentionalClose = false;
        return;
      }
      void appendKioskLog('warn', '[realtime] WebSocket disconnected unexpectedly');
      for (const handler of this.disconnectHandlers) {
        handler();
      }
      if (!this.disposed) this.scheduleReconnect();
    };

    socket.onerror = () => {
      if (gen !== this.connectionGen || this.ws !== socket) return;
      void appendKioskLog('warn', '[realtime] WebSocket error');
      socket.close();
    };
  }

  /** Replace the subscription set (used on reconnect / player login). */
  resetSubscriptions(channels: string[]): void {
    const previous = new Set(this.subscriptions);
    this.subscriptions.clear();
    for (const ch of channels) this.subscriptions.add(ch);

    const socket = this.ws;
    if (!socket || socket.readyState !== WebSocket.OPEN) return;

    const removed = [...previous].filter((ch) => !this.subscriptions.has(ch));
    const added = [...this.subscriptions].filter((ch) => !previous.has(ch));
    if (removed.length > 0) {
      this.sendOn(socket, { type: 'Unsubscribe', channels: removed });
    }
    if (added.length > 0) {
      this.sendOn(socket, { type: 'Subscribe', channels: added });
    }
  }

  subscribe(channels: string[]): void {
    for (const ch of channels) this.subscriptions.add(ch);
    const socket = this.ws;
    if (socket?.readyState === WebSocket.OPEN) {
      this.sendOn(socket, { type: 'Subscribe', channels });
    }
  }

  unsubscribe(channels: string[]): void {
    for (const ch of channels) this.subscriptions.delete(ch);
    const socket = this.ws;
    if (socket?.readyState === WebSocket.OPEN) {
      this.sendOn(socket, { type: 'Unsubscribe', channels });
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

  /** Fires whenever the socket opens, including after auto-reconnect. */
  onConnect(handler: () => void): () => void {
    this.connectHandlers.add(handler);
    return () => this.connectHandlers.delete(handler);
  }

  /** Fires when the socket closes unexpectedly (not intentional disconnect). */
  onDisconnect(handler: () => void): () => void {
    this.disconnectHandlers.add(handler);
    return () => this.disconnectHandlers.delete(handler);
  }

  disconnect(): void {
    this.disposed = true;
    this.reconnectAttempts = 0;
    this.connectionGen += 1;
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

  private sendOn(socket: WebSocket, data: Record<string, unknown>): void {
    if (socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(data));
    }
  }

  private scheduleReconnect(): void {
    if (this.disposed || this.reconnectTimer) return;
    const delay = Math.min(BASE_DELAY * 2 ** this.reconnectAttempts, MAX_DELAY);
    this.reconnectAttempts += 1;
    if (this.reconnectAttempts === 1 || this.reconnectAttempts % RECONNECT_LOG_EVERY === 0) {
      void appendKioskLog(
        'warn',
        `[realtime] reconnect attempt ${this.reconnectAttempts} in ${delay}ms`,
      );
    }
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }
}
