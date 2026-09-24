import { http } from '@gaming-cafe/utils';

export interface Configuration {
  id: string;
  key: string;
  value: unknown;
  category: string;
  description?: string | null;
  createdAt: string;
  updatedAt: string;
}

export const getConfigs = async (category?: string) =>
  http.get<Configuration[]>('/config', { params: category ? { category } : {} });

export const getConfig = async (key: string) => http.get<Configuration>(`/config/${key}`);

export const upsertConfig = async (key: string, value: unknown, description?: string) =>
  http.put<Configuration>(`/config/${key}`, { value, description });

export type SettingScope = 'organization' | 'location';
export type SettingValueType =
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'uuid'
  | 'timezone'
  | 'currency';

export interface SettingDefinition {
  key: string;
  category: string;
  description: string;
  valueType: SettingValueType;
  defaultValue: unknown;
  allowedScopes: SettingScope[];
  validation: Record<string, unknown>;
  sensitive: boolean;
  owner: string;
}

export interface ResolvedSetting {
  key: string;
  value: unknown;
  sourceScope: 'platform' | SettingScope;
  sourceId?: string | null;
  revision: number;
  updatedAt?: string | null;
  overridden: boolean;
}

export interface SettingOverride {
  id: string;
  organizationId: string;
  locationId?: string | null;
  key: string;
  value: unknown;
  revision: number;
  updatedAt: string;
}

export interface SettingRevision {
  id: number;
  key: string;
  locationId?: string | null;
  revision: number;
  operation: 'create' | 'update' | 'delete';
  oldValue?: unknown;
  newValue?: unknown;
  reason: string;
  actorUserId?: string | null;
  requestId?: string | null;
  createdAt: string;
}

export interface VenueLocation {
  id: string;
  organizationId: string;
  slug: string;
  name: string;
  timezone: string;
  currency: string;
  isActive: boolean;
}

export const getSettingCatalog = (organizationId: string) =>
  http.get<SettingDefinition[]>(`/organizations/${organizationId}/settings/catalog`);

export const getVenueLocations = (organizationId: string) =>
  http.get<VenueLocation[]>(`/organizations/${organizationId}/locations`);

export const getEffectiveSettings = (organizationId: string, locationId?: string) =>
  http.get<ResolvedSetting[]>(`/organizations/${organizationId}/settings/effective`, {
    params: locationId ? { locationId } : {},
  });

export const putSettingOverride = (
  organizationId: string,
  key: string,
  input: { locationId?: string; value: unknown; reason: string; expectedRevision: number },
) => http.put<SettingOverride>(`/organizations/${organizationId}/settings/overrides/${key}`, input);

export const deleteSettingOverride = (
  organizationId: string,
  key: string,
  input: { locationId?: string; expectedRevision: number; reason: string },
) =>
  http.delete<{ deleted: boolean }>(`/organizations/${organizationId}/settings/overrides/${key}`, {
    params: input,
  });

export const getSettingHistory = (organizationId: string, locationId?: string) =>
  http.get<SettingRevision[]>(`/organizations/${organizationId}/settings/history`, {
    params: locationId ? { locationId, limit: 100 } : { limit: 100 },
  });
