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
  GamesRead = 'games:read',
  GamesWrite = 'games:write',
  UnitsRead = 'units:read',
  UnitsWrite = 'units:write',
  DeviceGamesRead = 'device-games:read',
  DeviceGamesWrite = 'device-games:write',
  FilesRead = 'files:read',
  FilesWrite = 'files:write',
  ShiftsRead = 'shifts:read',
  ShiftsWrite = 'shifts:write',
  CashRegistersRead = 'cash-registers:read',
  CashRegistersWrite = 'cash-registers:write',
  ExpensesRead = 'expenses:read',
  ExpensesWrite = 'expenses:write',
  ExpensesApprove = 'expenses:approve',
  VendorsRead = 'vendors:read',
  VendorsWrite = 'vendors:write',
  ConfigRead = 'config:read',
  ConfigWrite = 'config:write',
}

export const ROLE_PERMISSIONS: Record<UserRole, Permission[]> = {
  admin: Object.values(Permission),
  staff: [
    Permission.StatsRead,
    Permission.DevicesRead,
    Permission.PlansRead,
    Permission.ProductsRead,
    Permission.GamesRead,
    Permission.UnitsRead,
    Permission.DeviceGamesRead,
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
    Permission.ExpensesRead,
    Permission.ExpensesWrite,
  ],
  player: [
    Permission.DevicesRead,
    Permission.PlansRead,
    Permission.ProductsRead,
    Permission.GamesRead,
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
