import { Permission } from '@gaming-cafe/contracts';
import { describe, expect, it } from 'vitest';
import { moduleRegistry } from '../constants/navItems';
import { filterNavItemsByPermission } from './filterNavItems';
import { getRouteTitle } from './routeTitle';

describe('panel module registry', () => {
  it('keeps procurement route titles aligned with navigation', () => {
    expect(getRouteTitle('/inventory/purchase-orders')).toBe('Purchase orders');
    expect(getRouteTitle('/inventory/purchase-orders/123')).toBe('Purchase order');
  });

  it('removes approval-only destinations when permission is absent', () => {
    const visible = filterNavItemsByPermission(moduleRegistry, (permission) =>
      [Permission.InventoryRead].includes(permission),
    );
    const inventory = visible.find((item) => item.path === '/inventory');
    expect(inventory?.children?.some((item) => item.path === '/inventory/purchase-orders')).toBe(
      false,
    );
  });
});
