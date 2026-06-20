/**
 * Shared user role and status types used across all gaming-cafe workspaces.
 */

export type UserRole = 'admin' | 'staff' | 'player';

export enum UserStatus {
  ACTIVE = 'ACTIVE',
  INACTIVE = 'INACTIVE',
  SUSPENDED = 'SUSPENDED',
}

export enum Permission {
  DevicesRead = 'devices:read',
  DevicesWrite = 'devices:write',
  PlansRead = 'plans:read',
  PlansWrite = 'plans:write',
  ProductsRead = 'products:read',
  ProductsWrite = 'products:write',
  SessionsRead = 'sessions:read',
  SessionsWrite = 'sessions:write',
  StatsRead = 'stats:read',
  TransactionsRead = 'transactions:read',
  TransactionsWrite = 'transactions:write',
  PlayerPlansRead = 'player-plans:read',
  PlayerPlansWrite = 'player-plans:write',
  PlayersRead = 'players:read',
  PlayersWrite = 'players:write',
  UnitsRead = 'units:read',
  UnitsWrite = 'units:write',
  ShiftsRead = 'shifts:read',
  ShiftsWrite = 'shifts:write',
  ShiftsForceClose = 'shifts:force_close',
  CashRegistersRead = 'cash-registers:read',
  CashRegistersWrite = 'cash-registers:write',
  CashRegistersReconcile = 'cash-registers:reconcile',
  CashRegistersAdjustOpening = 'cash-registers:adjust_opening',
  CashDepositsRead = 'cash-deposits:read',
  CashDepositsWrite = 'cash-deposits:write',
  CashDepositsApprove = 'cash-deposits:approve',
  CreditRead = 'credit:read',
  CreditWrite = 'credit:write',
  CreditLimitWrite = 'credit-limit:write',
  StaffGamingAllowanceRead = 'staff-gaming-allowance:read',
  StaffGamingAllowanceWrite = 'staff-gaming-allowance:write',
  ExpensesRead = 'expenses:read',
  ExpensesWrite = 'expenses:write',
  ExpensesApprove = 'expenses:approve',
  VendorsRead = 'vendors:read',
  VendorsWrite = 'vendors:write',
  ConfigRead = 'config:read',
  ConfigWrite = 'config:write',
  GamesRead = 'games:read',
  GamesWrite = 'games:write',
  InventoryRead = 'inventory:read',
  InventoryManage = 'inventory:manage',
  InventoryTransferRequest = 'inventory:transfer_request',
  InventoryTransferFulfill = 'inventory:transfer_fulfill',
  InventoryWasteRecord = 'inventory:waste_record',
  InventoryWasteApprove = 'inventory:waste_approve',
}

export const ROLE_PERMISSIONS: Record<UserRole, Permission[]> = {
  admin: Object.values(Permission).filter((permission) => permission !== Permission.ShiftsWrite),
  staff: [
    Permission.StatsRead,
    Permission.GamesRead,
    Permission.DevicesRead,
    Permission.PlansRead,
    Permission.ProductsRead,
    Permission.UnitsRead,
    Permission.SessionsRead,
    Permission.SessionsWrite,
    Permission.TransactionsRead,
    Permission.TransactionsWrite,
    Permission.PlayerPlansRead,
    Permission.PlayerPlansWrite,
    Permission.PlayersRead,
    Permission.PlayersWrite,
    Permission.ShiftsRead,
    Permission.ShiftsWrite,
    Permission.CashRegistersRead,
    Permission.CashRegistersWrite,
    Permission.CashDepositsRead,
    Permission.CashDepositsWrite,
    Permission.CreditRead,
    Permission.CreditWrite,
    Permission.ExpensesRead,
    Permission.InventoryRead,
    Permission.InventoryTransferRequest,
    Permission.InventoryWasteRecord,
  ],
  player: [
    Permission.GamesRead,
    Permission.DevicesRead,
    Permission.PlansRead,
    Permission.ProductsRead,
    Permission.SessionsRead,
    Permission.SessionsWrite,
    Permission.TransactionsRead,
    Permission.PlayerPlansRead,
  ],
};

/**
 * Type guard to check if a string is a valid UserRole
 */
export function isUserRole(value: string): value is UserRole {
  return value === 'admin' || value === 'staff' || value === 'player';
}

/**
 * Type guard to check if a string is a valid UserStatus
 */
export function isUserStatus(value: string): value is UserStatus {
  return Object.values(UserStatus).includes(value as UserStatus);
}

/**
 * Returns permissions for a role string (defaults to empty for unknown roles).
 */
export function permissionsForRole(role: string): Permission[] {
  if (isUserRole(role)) {
    return ROLE_PERMISSIONS[role];
  }
  return [];
}
