import { Permission } from '@gaming-cafe/contracts';

/** Default landing route after login, handover, or permission denial. */
export function getDefaultHomePath(can: (permission: Permission) => boolean): string {
  // Staff and admin dashboards both live at `/` (role-specific view inside DashboardPage).
  if (can(Permission.SessionsRead) || can(Permission.StatsRead)) {
    return '/';
  }
  if (can(Permission.TransactionsRead)) {
    return '/product-transactions';
  }
  if (can(Permission.PlayerPlansRead)) {
    return '/plan-transactions';
  }
  return '/';
}
