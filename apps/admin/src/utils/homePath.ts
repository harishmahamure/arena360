import { Permission } from '@gaming-cafe/contracts';

/** Default landing route after login, handover, or permission denial. */
export function getDefaultHomePath(can: (permission: Permission) => boolean): string {
  // Staff and admin dashboards both live at `/` (role-specific view inside DashboardPage).
  if (can(Permission.StatsRead)) {
    return '/';
  }
  if (can(Permission.TransactionsRead)) {
    return '/product-transactions';
  }
  if (can(Permission.PlayerPlansRead)) {
    return '/plan-transactions';
  }
  const destinations: [Permission, string][] = [
    [Permission.KitchenRead, '/kitchen'],
    [Permission.FinanceRead, '/finance/reports'],
    [Permission.AccessRead, '/access'],
    [Permission.LocationsRead, '/locations'],
    [Permission.SessionsRead, '/sessions'],
    [Permission.ProductsRead, '/products'],
    [Permission.PlansRead, '/plans'],
    [Permission.PlayersRead, '/players'],
    [Permission.InventoryRead, '/inventory/overview'],
    [Permission.ExpensesRead, '/expenses'],
    [Permission.CreditRead, '/credit'],
    [Permission.SettingsRead, '/settings'],
    [Permission.ShiftsRead, '/shifts'],
    [Permission.GamesRead, '/games'],
    [Permission.DevicesRead, '/devices'],
    [Permission.VendorsRead, '/vendors'],
    [Permission.CashRegistersRead, '/cash-registers'],
    [Permission.CashDepositsRead, '/cash-deposits'],
    [Permission.ProcurementRead, '/inventory/purchase-orders'],
  ];
  return destinations.find(([permission]) => can(permission))?.[1] ?? '/';
}
