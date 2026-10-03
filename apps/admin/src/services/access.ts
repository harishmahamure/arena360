import { Permission } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';
export interface AccessPermission {
  key: string;
  label: string;
  description: string;
}
export interface AccessModule {
  key: string;
  name: string;
  required: boolean;
  permissions: AccessPermission[];
}
export interface AccessRole {
  id: string;
  name: string;
  description: string;
  permissions: string[];
  isTemplate: boolean;
  revision: number;
  memberCount: number;
}
export interface AccessMember {
  id: string;
  username: string;
  name: string;
  active: boolean;
  revision: number;
  roleIds: string[];
  locationIds: string[];
  locationRoles?: { locationId: string; roleIds: string[] }[];
}
export interface ModuleState {
  key: string;
  enabled: boolean;
  revision: number;
}
export interface AuditEntry {
  id: number;
  action: string;
  targetId: string;
  actor: string;
  at: string;
  before: Record<string, unknown> | null;
  after: Record<string, unknown> | null;
}
export interface AccessSnapshot {
  catalog: AccessModule[];
  roles: AccessRole[];
  members: AccessMember[];
  modules: ModuleState[];
  audit: AuditEntry[];
}
export interface RoleDraft {
  id?: string;
  name: string;
  description: string;
  permissions: string[];
  isTemplate: boolean;
  expectedRevision?: number;
}
export const getAccess = () => http.get<AccessSnapshot>('/access');
export const getMyAccess = () =>
  http.get<{ roles: string[]; permissions: string[]; organizationAdmin: boolean }>('/access/self');
export const saveRole = ({ id, ...draft }: RoleDraft) =>
  id
    ? http.put<{ id: string }>(`/access/roles/${id}`, draft)
    : http.post<{ id: string }>('/access/roles', draft);
export const deleteRole = (role: AccessRole) =>
  http.post(`/access/roles/${role.id}/delete`, { expectedRevision: role.revision });
export const saveMember = (member: AccessMember) =>
  http.put(`/access/members/${member.id}`, {
    roleIds: member.locationRoles
      ? [...new Set(member.locationRoles.flatMap((scope) => scope.roleIds))]
      : member.roleIds,
    locationIds: member.locationIds,
    locationRoles: member.locationRoles,
    active: member.active,
    expectedRevision: member.revision,
  });
export const createMember = (draft: {
  username: string;
  password: string;
  roleIds: string[];
  locationIds: string[];
  locationRoles?: { locationId: string; roleIds: string[] }[];
}) =>
  http.post('/access/members', {
    ...draft,
    roleIds: draft.locationRoles
      ? [...new Set(draft.locationRoles.flatMap((scope) => scope.roleIds))]
      : draft.roleIds,
  });
export const saveModule = (module: ModuleState) =>
  http.put(`/access/modules/${module.key}`, {
    enabled: module.enabled,
    expectedRevision: module.revision,
  });
export const effectiveGrants = (roleIds: string[], roles: AccessRole[], modules: ModuleState[]) =>
  [
    ...new Set(
      roles
        .filter((role) => !role.isTemplate && roleIds.includes(role.id))
        .flatMap((role) => role.permissions),
    ),
  ]
    .filter(
      (permission) =>
        !modules.some((module) => !module.enabled && permission.startsWith(module.key + ':')),
    )
    .sort();

export function normalizeGrants(grants: string[]): string[] {
  const known = new Set<string>(Object.values(Permission));
  return [
    ...new Set(
      grants.flatMap((grant) => {
        const read = `${grant.split(':')[0]}:read`;
        return known.has(read) ? [grant, read] : [grant];
      }),
    ),
  ];
}
