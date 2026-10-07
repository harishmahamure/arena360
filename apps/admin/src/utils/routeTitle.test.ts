import { Permission } from '@gaming-cafe/contracts';
import { describe, expect, it } from 'vitest';
import { moduleRegistry } from '../constants/navItems';
import { analyticsNavigationPath } from './analyticsNavigation';
import { filterNavItemsByPermission } from './filterNavItems';
import { getRouteTitle } from './routeTitle';

describe('panel module registry', () => {
  it('shows location management to users with location access', () => {
    const visible = filterNavItemsByPermission(
      moduleRegistry,
      (permission) => permission === Permission.LocationsRead,
    );
    expect(visible.some((item) => item.path === '/locations')).toBe(true);
    expect(getRouteTitle('/locations')).toBe('Locations');
  });

  it('groups the reporting subpages under the permission-protected business dashboard', () => {
    const allowed = filterNavItemsByPermission(moduleRegistry, (p) => p === Permission.FinanceRead);
    const business = allowed.find((item) => item.path === '/analytics');
    expect(business?.children).toHaveLength(11);
    expect(getRouteTitle('/analytics')).toBe('Business dashboard');
    expect(getRouteTitle('/analytics/retention')).toBe('Customer Retention');
    expect(
      filterNavItemsByPermission(moduleRegistry, () => false).some((i) => i.path === '/analytics'),
    ).toBe(false);
  });

  it('carries only reporting filters between subpages and leaves other modules alone', () => {
    const location = {
      pathname: '/analytics/executive',
      search: '?startDate=2026-09-01&endDate=2026-09-14&compare=false&active=true',
    };
    expect(analyticsNavigationPath('/analytics/retention', location)).toBe(
      '/analytics/retention?startDate=2026-09-01&endDate=2026-09-14&compare=false',
    );
    expect(analyticsNavigationPath('/analytics', location)).toBe(
      '/analytics?startDate=2026-09-01&endDate=2026-09-14&compare=false',
    );
    expect(analyticsNavigationPath('/sessions?active=true', location)).toBe(
      '/sessions?active=true',
    );
    expect(
      analyticsNavigationPath('/analytics/retention', {
        ...location,
        pathname: '/finance/reports',
      }),
    ).toBe('/analytics/retention');
  });
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
