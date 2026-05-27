import { ApiError } from './ApiError';
import type { ErrorEnvelope, SuccessEnvelope } from './types';

function isSuccessEnvelope<T>(value: unknown): value is SuccessEnvelope<T> {
  return (
    typeof value === 'object' &&
    value !== null &&
    'success' in value &&
    (value as SuccessEnvelope<T>).success === true &&
    'data' in value
  );
}

function isErrorEnvelope(value: unknown): value is ErrorEnvelope {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const record = value as Record<string, unknown>;
  return (
    typeof record.statusCode === 'number' &&
    typeof record.message === 'string' &&
    typeof record.error === 'string' &&
    record.success !== true
  );
}

/**
 * Unwrap a backend success envelope and return the inner `data` payload.
 * Throws {@link ApiError} when given an error envelope body.
 */
export function unwrapEnvelope<T>(body: unknown): T {
  if (isSuccessEnvelope<T>(body)) {
    return body.data;
  }

  if (isErrorEnvelope(body)) {
    throw ApiError.fromErrorEnvelope(body);
  }

  throw new ApiError({
    message: 'Unexpected API response shape',
    statusCode: 500,
  });
}
