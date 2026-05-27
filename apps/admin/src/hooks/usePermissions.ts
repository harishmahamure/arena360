import { Permission, permissionsForRole, type UserRole } from '@gaming-cafe/contracts';
import { useSelector } from './store';

export function usePermissions() {
  const role = useSelector((state) => state.auth.role) as UserRole | '';
  const permissions = role ? permissionsForRole(role) : [];

  const can = (permission: Permission) => permissions.includes(permission);

  return {
    role,
    permissions,
    can,
    isAdmin: role === 'admin',
    isStaff: role === 'staff',
  };
}

export { Permission };
