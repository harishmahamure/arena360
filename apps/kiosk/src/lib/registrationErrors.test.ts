import { ErrorCode } from '@gaming-cafe/contracts';
import { ApiError } from '@gaming-cafe/utils';
import { describe, expect, it } from 'vitest';
import { mapRegistrationApiError } from './registrationErrors';

describe('mapRegistrationApiError', () => {
  it('maps username conflict to the username field', () => {
    const error = ApiError.fromErrorEnvelope({
      statusCode: 409,
      message: ErrorCode.AUTH_USERNAME_ALREADY_EXISTS,
      error: 'Conflict',
      details: { field: 'username' },
    });

    expect(mapRegistrationApiError(error)).toEqual({
      username: 'That username is already taken',
    });
  });

  it('maps weak password to the password field', () => {
    const error = ApiError.fromErrorEnvelope({
      statusCode: 400,
      message: ErrorCode.AUTH_WEAK_PASSWORD,
      error: 'Bad Request',
      details: { field: 'password' },
    });

    expect(mapRegistrationApiError(error)).toEqual({
      password: 'Password does not meet security requirements',
    });
  });

  it('maps rate limit to a form banner', () => {
    const error = ApiError.fromErrorEnvelope({
      statusCode: 429,
      message: ErrorCode.REGISTRATION_RATE_LIMITED,
      error: 'Too Many Requests',
    });

    expect(mapRegistrationApiError(error)).toEqual({
      form: 'Too many registration attempts. Please try again later.',
    });
  });
});
