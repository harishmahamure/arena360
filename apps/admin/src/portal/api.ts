export type Tenant = {
  id: string;
  name: string;
  slug: string;
  state: string;
  is_enabled: boolean;
  timezone: string;
  owner_cell: string | null;
  cell_name: string | null;
  ownership_generation: number;
  schema_version: number;
  lease_fresh: boolean | null;
  lease_expires_at: string | null;
  created_at: string;
};
export type Cell = {
  id: string;
  name: string;
  address: string;
  state: string;
  tenant_count: number;
  hydration_ready: boolean | null;
};
export type Plan = {
  code: string;
  name: string;
  entitlements: Record<string, unknown>;
  graceDays: number;
  isActive: boolean;
};
export type Overview = {
  counts: { total: number; active: number; cold: number; attention: number };
  localCellId: string | null;
  canProvision: boolean;
  targetSchemaVersion: number;
};
export type Detail = {
  tenant: Tenant;
  subscription: {
    planCode: string;
    status: string;
    startsAt: string;
    endsAt: string | null;
  } | null;
  jobs: {
    id: string;
    kind: string;
    state: string;
    created_at: string;
    last_error: string | null;
  }[];
  admins: { id: string; username: string; active: boolean }[];
  licenses: {
    revision: number;
    status: string;
    valid_until: string;
    grace_until: string;
    entitlements: Record<string, unknown>;
  }[];
};
export class PortalClient {
  constructor(
    private token: string,
    private base = import.meta.env.VITE_PLATFORM_API_BASE ?? '',
  ) {}
  async request<T>(path: string, method = 'GET', body?: unknown, signal?: AbortSignal): Promise<T> {
    const response = await fetch(`${this.base.replace(/\/$/, '')}/platform${path}`, {
      method,
      signal,
      credentials: 'omit',
      headers: {
        ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
        ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const data = await response.json().catch(() => null);
    if (!response.ok)
      throw new Error(
        data?.details?.message ??
          data?.message ??
          data?.error ??
          data?.code ??
          `Request failed (${response.status})`,
      );
    return data as T;
  }
  async auth<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(`/auth${path}`, 'POST', body);
  }
}
