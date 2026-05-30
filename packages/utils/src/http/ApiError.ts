import { type ErrorCode, isErrorCode } from '@gaming-cafe/contracts';

export class ApiError extends Error {
  readonly statusCode: number;
  readonly code?: ErrorCode | string;
  readonly errorLabel?: string;
  readonly timestamp?: string;
  /** Structured error details from the server envelope (object or array). */
  readonly details?: unknown;

  constructor(options: {
    message: string;
    statusCode: number;
    code?: ErrorCode | string;
    errorLabel?: string;
    timestamp?: string;
    details?: unknown;
    cause?: unknown;
  }) {
    super(options.message, options.cause !== undefined ? { cause: options.cause } : undefined);
    this.name = 'ApiError';
    this.statusCode = options.statusCode;
    this.code = options.code;
    this.errorLabel = options.errorLabel;
    this.timestamp = options.timestamp;
    this.details = options.details;
  }

  static fromErrorEnvelope(
    body: {
      statusCode: number;
      message: string;
      error: string;
      timestamp?: string;
      details?: unknown;
    },
    cause?: unknown,
  ): ApiError {
    const code = isErrorCode(body.message) ? body.message : undefined;

    return new ApiError({
      message: body.message,
      statusCode: body.statusCode,
      code,
      errorLabel: body.error,
      timestamp: body.timestamp,
      details: body.details,
      cause,
    });
  }
}

export function isApiError(value: unknown): value is ApiError {
  return value instanceof ApiError;
}
