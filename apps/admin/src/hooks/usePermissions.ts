import { Permission } from '@gaming-cafe/contracts';
import { panelClaims, sessionPermissions } from '../lib/authSession';
import { useSelector } from './store';

export function usePermissions() {
  useSelector((state) => state.auth);
  const claims = panelClaims();
  const role = claims ? (claims.roles?.includes('admin') ? 'admin' : 'staff') : '';
  const permissions = sessionPermissions(claims);

  const can = (permission: Permission) => permissions.includes(permission);

  return {
    role,
    permissions,
    can,
    isAdmin: can(Permission.AccessManage),
    isStaff: can(Permission.ShiftsWrite),
  };
}

export { Permission };
