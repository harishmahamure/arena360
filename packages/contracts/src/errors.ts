/**
 * Shared error codes used across all gaming-cafe workspaces.
 * Backend, admin, and kiosk all import from this package.
 */

export enum ErrorCode {
  // Plan-related errors
  PLAN_EXPIRED = 'PLAN_EXPIRED',
  PLAN_EXHAUSTED = 'PLAN_EXHAUSTED',
  PLAN_NOT_ACTIVATED = 'PLAN_NOT_ACTIVATED',
  PLAN_CANCELLED = 'PLAN_CANCELLED',

  // Credit/session errors
  INSUFFICIENT_CREDITS = 'INSUFFICIENT_CREDITS',
  INSUFFICIENT_SESSIONS = 'INSUFFICIENT_SESSIONS',
  TIME_WINDOW_VIOLATION = 'TIME_WINDOW_VIOLATION',

  // Plan restriction errors
  DEVICE_TYPE_NOT_ALLOWED = 'DEVICE_TYPE_NOT_ALLOWED',
  MAX_SESSIONS_EXCEEDED = 'MAX_SESSIONS_EXCEEDED',
  COOLDOWN_ACTIVE = 'COOLDOWN_ACTIVE',

  // Device errors
  DEVICE_UNDER_MAINTENANCE = 'DEVICE_UNDER_MAINTENANCE',
  DEVICE_OUT_OF_SERVICE = 'DEVICE_OUT_OF_SERVICE',
  DEVICE_NOT_OPERATIONAL = 'DEVICE_NOT_OPERATIONAL',

  // Session errors
  ACTIVE_SESSION_EXISTS = 'ACTIVE_SESSION_EXISTS',
  SESSION_NOT_FOUND = 'SESSION_NOT_FOUND',
  SESSION_ALREADY_ENDED = 'SESSION_ALREADY_ENDED',

  // Payment/transaction errors
  PAYMENT_FAILED = 'PAYMENT_FAILED',
  TRANSACTION_ALREADY_PROCESSED = 'TRANSACTION_ALREADY_PROCESSED',
  INVALID_PAYMENT_STATUS = 'INVALID_PAYMENT_STATUS',
  REFUND_NOT_ALLOWED = 'REFUND_NOT_ALLOWED',

  // Configuration errors
  INVALID_PLAN_CONFIGURATION = 'INVALID_PLAN_CONFIGURATION',
  INVALID_TIME_WINDOW = 'INVALID_TIME_WINDOW',
  INVALID_PLAN_TYPE = 'INVALID_PLAN_TYPE',

  // Auth errors
  AUTH_INVALID_CREDENTIALS = 'AUTH_INVALID_CREDENTIALS',
  AUTH_TOKEN_EXPIRED = 'AUTH_TOKEN_EXPIRED',
  AUTH_TOKEN_INVALID = 'AUTH_TOKEN_INVALID',
  AUTH_INSUFFICIENT_PERMISSIONS = 'AUTH_INSUFFICIENT_PERMISSIONS',
  AUTH_EMAIL_ALREADY_EXISTS = 'AUTH_EMAIL_ALREADY_EXISTS',
  AUTH_WEAK_PASSWORD = 'AUTH_WEAK_PASSWORD',
}

export const ErrorMessages: Record<ErrorCode, string> = {
  [ErrorCode.PLAN_EXPIRED]: 'The plan has expired and can no longer be used',
  [ErrorCode.PLAN_EXHAUSTED]: 'The plan has been exhausted (no remaining credits or sessions)',
  [ErrorCode.PLAN_NOT_ACTIVATED]: 'The plan has not been activated yet',
  [ErrorCode.PLAN_CANCELLED]: 'The plan has been cancelled',

  [ErrorCode.INSUFFICIENT_CREDITS]: 'Insufficient time credits remaining in the plan',
  [ErrorCode.INSUFFICIENT_SESSIONS]: 'No remaining sessions available in the plan',
  [ErrorCode.TIME_WINDOW_VIOLATION]:
    'Current time is outside the allowed time window for this plan',

  [ErrorCode.DEVICE_TYPE_NOT_ALLOWED]: 'This device type is not allowed for your plan',
  [ErrorCode.MAX_SESSIONS_EXCEEDED]: 'Maximum concurrent sessions limit exceeded',
  [ErrorCode.COOLDOWN_ACTIVE]:
    'Cooldown period is active, please wait before starting a new session',

  [ErrorCode.DEVICE_UNDER_MAINTENANCE]: 'The device is currently under maintenance',
  [ErrorCode.DEVICE_OUT_OF_SERVICE]: 'The device is out of service',
  [ErrorCode.DEVICE_NOT_OPERATIONAL]: 'The device is not operational',

  [ErrorCode.ACTIVE_SESSION_EXISTS]: 'An active session already exists for this player plan',
  [ErrorCode.SESSION_NOT_FOUND]: 'Session not found',
  [ErrorCode.SESSION_ALREADY_ENDED]: 'Session has already been ended',

  [ErrorCode.PAYMENT_FAILED]: 'Payment processing failed',
  [ErrorCode.TRANSACTION_ALREADY_PROCESSED]: 'Transaction has already been processed',
  [ErrorCode.INVALID_PAYMENT_STATUS]: 'Invalid payment status transition',
  [ErrorCode.REFUND_NOT_ALLOWED]: 'Refund is not allowed for this transaction',

  [ErrorCode.INVALID_PLAN_CONFIGURATION]:
    'Plan configuration is invalid for the specified plan type',
  [ErrorCode.INVALID_TIME_WINDOW]: 'Time window start must be before time window end',
  [ErrorCode.INVALID_PLAN_TYPE]: 'Invalid plan type specified',

  [ErrorCode.AUTH_INVALID_CREDENTIALS]: 'Invalid email or password',
  [ErrorCode.AUTH_TOKEN_EXPIRED]: 'Authentication token has expired',
  [ErrorCode.AUTH_TOKEN_INVALID]: 'Authentication token is invalid',
  [ErrorCode.AUTH_INSUFFICIENT_PERMISSIONS]: 'Insufficient permissions to access this resource',
  [ErrorCode.AUTH_EMAIL_ALREADY_EXISTS]: 'An account with this email already exists',
  [ErrorCode.AUTH_WEAK_PASSWORD]: 'Password does not meet security requirements',
};

/**
 * Type guard to check if a string is a valid ErrorCode
 */
export function isErrorCode(value: string): value is ErrorCode {
  return Object.values(ErrorCode).includes(value as ErrorCode);
}

/**
 * Get the error message for a given error code
 */
export function getErrorMessage(code: ErrorCode): string {
  return ErrorMessages[code];
}
