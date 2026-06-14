import { adminNavItems } from '../constants/navItems';

const EXACT_TITLES: Record<string, string> = {
  '/': 'Dashboard',
  '/sessions': 'Sessions',
  '/sessions/new': 'Start session',
  '/players': 'Players',
  '/players/new': 'Add player',
  '/plan-transactions': 'Plan sales',
  '/plan-transactions/new': 'Buy plan',
  '/product-transactions': 'POS sales',
  '/product-transactions/new': 'POS',
  '/products': 'Products',
  '/products/new': 'Add product',
  '/inventory/locations': 'Inventory locations',
  '/inventory/warehouse': 'Warehouse stock',
  '/inventory/transfers': 'Transfer requests',
  '/inventory/transfers/new': 'New transfer',
  '/inventory/waste': 'Waste approvals',
  '/inventory/waste/new': 'Record waste',
  '/inventory/waste/report': 'Waste report',
  '/games': 'Games',
  '/games/new': 'Add game',
  '/devices': 'Devices',
  '/devices/new': 'Add device',
  '/plans': 'Plans',
  '/plans/new': 'Add plan',
  '/shifts': 'Shifts',
  '/cash-registers': 'Cash registers',
  '/cash-deposits': 'Cash deposits',
  '/credit': 'Running tab',
  '/expenses': 'Expenses',
  '/expenses/new': 'Add expense',
  '/vendors': 'Vendors',
  '/vendors/new': 'Add vendor',
  '/settings': 'Settings',
};

const DETAIL_TITLES: Array<{ prefix: string; title: string }> = [
  { prefix: '/sessions/', title: 'Session' },
  { prefix: '/players/', title: 'Player' },
  { prefix: '/plan-transactions/', title: 'Plan sale' },
  { prefix: '/product-transactions/', title: 'POS sale' },
  { prefix: '/products/', title: 'Product' },
  { prefix: '/games/', title: 'Game' },
  { prefix: '/devices/', title: 'Device' },
  { prefix: '/plans/', title: 'Plan' },
  { prefix: '/shifts/', title: 'Shift' },
  { prefix: '/cash-registers/', title: 'Cash register' },
  { prefix: '/expenses/', title: 'Expense' },
  { prefix: '/vendors/', title: 'Vendor' },
];

function matchNavParentTitle(pathname: string): string | undefined {
  let best: { path: string; title: string } | undefined;
  for (const item of adminNavItems) {
    if (item.path === '/') continue;
    if (pathname === item.path || pathname.startsWith(`${item.path}/`)) {
      if (!best || item.path.length > best.path.length) {
        best = { path: item.path, title: item.title };
      }
    }
  }
  return best?.title;
}

export function getRouteTitle(pathname: string): string {
  if (EXACT_TITLES[pathname]) {
    return EXACT_TITLES[pathname];
  }

  for (const { prefix, title } of DETAIL_TITLES) {
    if (pathname.startsWith(prefix) && pathname.length > prefix.length) {
      return title;
    }
  }

  const parentTitle = matchNavParentTitle(pathname);
  if (parentTitle) {
    return parentTitle;
  }

  return pathname === '/' ? 'Dashboard' : 'Dashboard';
}
