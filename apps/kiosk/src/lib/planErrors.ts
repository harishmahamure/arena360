import { ErrorCode, getErrorMessage, isErrorCode } from '@gaming-cafe/contracts';

const RECHARGE_HEADLINE = 'No usable plan on this station. Please recharge at the counter.';

const STAFF_ALLOWANCE_CODES: ReadonlySet<string> = new Set([
  ErrorCode.STAFF_SHIFT_ACTIVE,
  ErrorCode.STAFF_ALLOWANCE_NONE,
  ErrorCode.STAFF_ALLOWANCE_EXPIRED,
  ErrorCode.STAFF_ALLOWANCE_EXHAUSTED,
]);

const PLAN_RELATED_CODES: ReadonlySet<string> = new Set([
  ErrorCode.PLAN_EXPIRED,
  ErrorCode.PLAN_EXHAUSTED,
  ErrorCode.PLAN_NOT_ACTIVATED,
  ErrorCode.PLAN_CANCELLED,
  ErrorCode.INSUFFICIENT_CREDITS,
  ErrorCode.INSUFFICIENT_SESSIONS,
  ErrorCode.TIME_WINDOW_VIOLATION,
  ErrorCode.DEVICE_TYPE_NOT_ALLOWED,
]);

export function isPlanRelatedErrorCode(code: string): boolean {
  return PLAN_RELATED_CODES.has(code);
}

export function formatPlayerLoginError(message: string): string {
  if (message === ErrorCode.PLAYER_ALREADY_IN_SESSION) {
    return 'You are already logged in on another device.';
  }
  if (message === ErrorCode.AUTH_INVALID_CREDENTIALS) {
    return getErrorMessage(ErrorCode.AUTH_INVALID_CREDENTIALS);
  }
  if (isErrorCode(message) && STAFF_ALLOWANCE_CODES.has(message)) {
    return getErrorMessage(message);
  }
  if (isErrorCode(message) && isPlanRelatedErrorCode(message)) {
    return `${RECHARGE_HEADLINE}\n${getErrorMessage(message)}`;
  }
  return message;
}
