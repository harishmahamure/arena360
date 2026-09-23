import type { Permission } from '@gaming-cafe/contracts';
import type { NavItem } from '@gaming-cafe/ui';

export interface AdminNavChildItem {
  id?: string;
  title: string;
  path: string;
  requiredPermission?: Permission;
  breadcrumbs?: string[];
  searchKeywords?: string[];
}

export interface AdminNavItem extends NavItem {
  id?: string;
  requiredPermission?: Permission;
  children?: AdminNavChildItem[];
  breadcrumbs?: string[];
  searchKeywords?: string[];
}

export function filterNavItemsByPermission(
  items: AdminNavItem[],
  can: (permission: Permission) => boolean,
): NavItem[] {
  return items
    .filter((item) => !item.requiredPermission || can(item.requiredPermission))
    .map((item) => {
      if (!item.children?.length) {
        return item;
      }

      const children = item.children.filter(
        (child) => !child.requiredPermission || can(child.requiredPermission),
      );

      if (children.length === 0) {
        return null;
      }

      return {
        ...item,
        children,
      };
    })
    .filter((item): item is AdminNavItem => item !== null);
}
