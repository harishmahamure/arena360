/**
 * Shared user role and status types used across all gaming-cafe workspaces.
 */

export type UserRole = 'admin' | 'player';

export enum UserStatus {
  ACTIVE = 'ACTIVE',
  INACTIVE = 'INACTIVE',
  SUSPENDED = 'SUSPENDED',
}

/**
 * Type guard to check if a string is a valid UserRole
 */
export function isUserRole(value: string): value is UserRole {
  return value === 'admin' || value === 'player';
}

/**
 * Type guard to check if a string is a valid UserStatus
 */
export function isUserStatus(value: string): value is UserStatus {
  return Object.values(UserStatus).includes(value as UserStatus);
}
