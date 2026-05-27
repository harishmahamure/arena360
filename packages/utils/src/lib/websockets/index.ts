import { io, type Socket } from 'socket.io-client';

// Type definitions for events
interface EventEnvelope {
  eventId: string;
  payload: unknown;
  timestamp: number;
}

interface RoomJoinedEvent {
  rooms: string[];
}

interface RoomErrorEvent {
  room: string;
  error: string;
}

interface RoomLeftEvent {
  room: string;
}

interface RoomResponse {
  success: boolean;
  room?: string;
  error?: string;
}

interface SystemShutdownEvent {
  message: string;
  reconnectIn: number;
}

export class WebSocketClient {
  private socket: Socket | null = null;
  private processedEvents = new Set<string>();
  private eventHandlers = new Map<string, Array<(payload: unknown) => void>>();
  private maxProcessedEvents = 10000;

  constructor(
    private readonly gatewayUrl: string,
    private readonly namespace: '/public' | '/private' | '/admin' = '/private',
  ) {}

  connect(token: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.socket = io(`${this.gatewayUrl}${this.namespace}`, {
        auth: { token },
        transports: ['websocket'],
        reconnection: true,
        reconnectionAttempts: Infinity,
        reconnectionDelay: 1000,
        reconnectionDelayMax: 5000,
        timeout: 20000,
      });

      this.socket.on('connect', () => {
        console.log('[Gateway] Connected:', this.socket?.id);
        resolve();
      });

      this.socket.on('connect_error', (error) => {
        console.error('[Gateway] Connection error:', error.message);
        reject(error);
      });

      this.setupEventHandlers();
    });
  }

  disconnect(): void {
    if (this.socket) {
      this.socket.disconnect();
      this.socket = null;
    }
  }

  isConnected(): boolean {
    return this.socket?.connected ?? false;
  }

  async joinRoom(room: string): Promise<RoomResponse> {
    if (!this.socket) {
      throw new Error('Not connected');
    }

    return new Promise((resolve) => {
      this.socket!.emit('room:join', { room }, (response: RoomResponse) => {
        if (response.success) {
          console.log('[Gateway] Joined room:', room);
        } else {
          console.warn('[Gateway] Failed to join room:', room, response.error);
        }
        resolve(response);
      });
    });
  }

  async leaveRoom(room: string): Promise<RoomResponse> {
    if (!this.socket) {
      throw new Error('Not connected');
    }

    return new Promise((resolve) => {
      this.socket!.emit('room:leave', { room }, (response: RoomResponse) => {
        console.log('[Gateway] Left room:', room);
        resolve(response);
      });
    });
  }

  /**
   * Measure latency to server
   */
  async ping(): Promise<number> {
    if (!this.socket) {
      throw new Error('Not connected');
    }

    const start = Date.now();
    return new Promise((resolve) => {
      this.socket!.emit('ping', () => {
        resolve(Date.now() - start);
      });
    });
  }

  /**
   * Subscribe to an event type
   */
  on<T = unknown>(event: string, handler: (payload: T) => void): () => void {
    const handlers = this.eventHandlers.get(event) ?? [];
    handlers.push(handler as (payload: unknown) => void);
    this.eventHandlers.set(event, handlers);

    // Return unsubscribe function
    return () => {
      const h = this.eventHandlers.get(event) ?? [];
      const index = h.indexOf(handler as (payload: unknown) => void);
      if (index !== -1) {
        h.splice(index, 1);
      }
    };
  }

  /**
   * Setup internal event handlers
   */
  private setupEventHandlers(): void {
    if (!this.socket) return;

    // Room events
    this.socket.on('room:joined', (data: RoomJoinedEvent) => {
      console.log('[Gateway] Joined rooms:', data.rooms);
    });

    this.socket.on('room:error', (data: RoomErrorEvent) => {
      console.error('[Gateway] Room error:', data.room, data.error);
    });

    this.socket.on('room:left', (data: RoomLeftEvent) => {
      console.log('[Gateway] Left room:', data.room);
    });

    // System events
    this.socket.on('system:shutdown', (data: SystemShutdownEvent) => {
      console.warn('[Gateway] Server shutting down:', data.message);
    });

    // Reconnection handling
    this.socket.on('disconnect', (reason) => {
      console.log('[Gateway] Disconnected:', reason);
    });

    this.socket.on('reconnect', (attemptNumber) => {
      console.log('[Gateway] Reconnected after', attemptNumber, 'attempts');
      // Trigger state refresh
      this.eventHandlers.get('_reconnect')?.forEach((h) => {
        h(null);
      });
    });

    // Catch-all for business events
    this.socket.onAny((event: string, envelope: EventEnvelope) => {
      // Skip internal events
      if (
        event.startsWith('room:') ||
        event.startsWith('system:') ||
        event === 'connect' ||
        event === 'disconnect'
      ) {
        return;
      }

      // Client-side deduplication
      if (this.processedEvents.has(envelope.eventId)) {
        console.debug('[Gateway] Skipping duplicate event:', envelope.eventId);
        return;
      }

      // Track processed event
      this.processedEvents.add(envelope.eventId);
      this.cleanupProcessedEvents();

      // Dispatch to handlers
      const handlers = this.eventHandlers.get(event) ?? [];
      for (const handler of handlers) {
        try {
          handler(envelope.payload);
        } catch (error) {
          console.error('[Gateway] Event handler error:', event, error);
        }
      }
    });
  }

  /**
   * Cleanup old processed events to prevent memory leak
   */
  private cleanupProcessedEvents(): void {
    if (this.processedEvents.size > this.maxProcessedEvents) {
      const toDelete = this.processedEvents.size - this.maxProcessedEvents / 2;
      const iterator = this.processedEvents.values();
      for (let i = 0; i < toDelete; i++) {
        const value = iterator.next().value;
        if (value) {
          this.processedEvents.delete(value);
        }
      }
    }
  }
}

// ============================================================================
// Lazy Singleton Factory
// ============================================================================

type Namespace = '/public' | '/private' | '/admin';

const clients: Partial<Record<Namespace, WebSocketClient>> = {};

/**
 * Get a shared WebSocketClient instance for the given namespace.
 * Lazily creates the client on first access.
 * Safe for SSR - throws if called on server.
 */
export function getWebSocketClient(namespace: Namespace = '/private'): WebSocketClient {
  if (typeof window === 'undefined') {
    throw new Error('WebSocketClient can only be used in browser environment');
  }

  if (!clients[namespace]) {
    const gatewayUrl = import.meta.env.VITE_GATEWAY_URL;
    if (!gatewayUrl) {
      throw new Error('VITE_GATEWAY_URL environment variable is not set');
    }
    clients[namespace] = new WebSocketClient(gatewayUrl, namespace);
  }

  return clients[namespace];
}

/**
 * Disconnect and remove a specific client instance.
 * Useful for cleanup or reconnecting with different auth.
 */
export function disconnectWebSocketClient(namespace: Namespace): void {
  const client = clients[namespace];
  if (client) {
    client.disconnect();
    delete clients[namespace];
  }
}

/**
 * Disconnect all WebSocket clients.
 */
export function disconnectAllWebSocketClients(): void {
  (Object.keys(clients) as Namespace[]).forEach((ns) => {
    disconnectWebSocketClient(ns);
  });
}
