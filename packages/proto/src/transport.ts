import { type Client, createClient, type Transport } from '@connectrpc/connect';
import { createGrpcWebTransport } from '@connectrpc/connect-web';
import {
  AuthService,
  ConfigurationService,
  DeviceService,
  InventoryService,
  NotificationService,
  PlanService,
  type ProxyResponse,
  SessionService,
  ShiftCashService,
  StatisticsService,
  TransactionService,
  UploadService,
  UserService,
} from './gen/arena360/v1/api_pb';

export interface ProtobufRequest {
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  path: string;
  query?: string;
  body?: Uint8Array;
  headers?: Record<string, string>;
  timeoutMs?: number;
}

type DomainClient = Client<typeof AuthService>;
const textEncoder = new TextEncoder();
const DOMAIN_SERVICES = {
  auth: AuthService,
  users: UserService,
  devices: DeviceService,
  plans: PlanService,
  sessions: SessionService,
  transactions: TransactionService,
  shifts: ShiftCashService,
  inventory: InventoryService,
  notifications: NotificationService,
  config: ConfigurationService,
  stats: StatisticsService,
  uploads: UploadService,
} as const;

function domainForPath(path: string): keyof typeof DOMAIN_SERVICES {
  const segment = path.split('/').filter(Boolean)[0] ?? '';
  if (segment === 'auth') return 'auth';
  if (['users', 'credit', 'staff-gaming-allowances'].includes(segment)) return 'users';
  if (['devices', 'units', 'games'].includes(segment)) return 'devices';
  if (['plans', 'player-plans', 'balances'].includes(segment)) return 'plans';
  if (['sessions', 'kiosk', 'kiosk-orders', 'realtime'].includes(segment)) return 'sessions';
  if (['transactions', 'products'].includes(segment)) return 'transactions';
  if (['shifts', 'cash-registers', 'cash-deposits', 'expenses', 'vendors'].includes(segment))
    return 'shifts';
  if (segment === 'inventory') return 'inventory';
  if (['notifications', 'activity-log'].includes(segment)) return 'notifications';
  if (segment === 'config') return 'config';
  if (segment === 'stats') return 'stats';
  return 'uploads';
}

export function createArena360GrpcWebTransport(baseUrl: string): {
  invoke(request: ProtobufRequest): Promise<ProxyResponse>;
} {
  const transport: Transport = createGrpcWebTransport({
    baseUrl: baseUrl.replace(/\/$/, ''),
    useBinaryFormat: true,
  });
  const clients = new Map<keyof typeof DOMAIN_SERVICES, DomainClient>();
  return {
    async invoke(request) {
      const domain = domainForPath(request.path);
      let client = clients.get(domain);
      if (!client) {
        client = createClient(DOMAIN_SERVICES[domain], transport) as DomainClient;
        clients.set(domain, client);
      }
      return client.invoke(
        {
          method: request.method,
          path: request.path,
          query: request.query ?? '',
          bodyJson: request.body ?? new Uint8Array(),
          headers: request.headers ?? {},
        },
        {
          ...(request.headers ? { headers: request.headers } : {}),
          ...(request.timeoutMs === undefined ? {} : { timeoutMs: request.timeoutMs }),
        },
      );
    },
  };
}

export function encodeJson(value: unknown): Uint8Array {
  return value === undefined ? new Uint8Array() : textEncoder.encode(JSON.stringify(value));
}
