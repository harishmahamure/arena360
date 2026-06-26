import { ErrorCode, getErrorMessage, isErrorCode } from '@gaming-cafe/contracts';
import type { ApiError } from '@gaming-cafe/utils';

export type RegistrationField =
  | 'username'
  | 'password'
  | 'confirmPassword'
  | 'phoneNumber'
  | 'firstName'
  | 'lastName';

export type RegistrationFormErrors = Partial<Record<RegistrationField, string>> & {
  form?: string;
};

function fieldFromDetails(details: unknown): RegistrationField | undefined {
  if (!details || typeof details !== 'object') return undefined;
  const field = (details as { field?: unknown }).field;
  if (typeof field !== 'string') return undefined;
  const allowed: RegistrationField[] = [
    'username',
    'password',
    'confirmPassword',
    'phoneNumber',
    'firstName',
    'lastName',
  ];
  return allowed.includes(field as RegistrationField) ? (field as RegistrationField) : undefined;
}

function messageForCode(code: string): string {
  if (isErrorCode(code)) {
    return getErrorMessage(code);
  }
  return code;
}

export function mapRegistrationApiError(error: ApiError): RegistrationFormErrors {
  const field = fieldFromDetails(error.details);
  const message = messageForCode(error.message);

  if (error.code === ErrorCode.AUTH_USERNAME_ALREADY_EXISTS || field === 'username') {
    return {
      username:
        error.code === ErrorCode.AUTH_USERNAME_ALREADY_EXISTS
          ? getErrorMessage(ErrorCode.AUTH_USERNAME_ALREADY_EXISTS)
          : message,
    };
  }

  if (error.code === ErrorCode.AUTH_WEAK_PASSWORD || field === 'password') {
    return {
      password:
        error.code === ErrorCode.AUTH_WEAK_PASSWORD
          ? getErrorMessage(ErrorCode.AUTH_WEAK_PASSWORD)
          : message,
    };
  }

  if (field === 'phoneNumber') {
    return { phoneNumber: message };
  }

  if (field === 'firstName') {
    return { firstName: message };
  }

  if (field === 'lastName') {
    return { lastName: message };
  }

  if (error.code === ErrorCode.REGISTRATION_RATE_LIMITED) {
    return { form: getErrorMessage(ErrorCode.REGISTRATION_RATE_LIMITED) };
  }

  if (error.code === ErrorCode.DEVICE_UNDER_MAINTENANCE) {
    return { form: getErrorMessage(ErrorCode.DEVICE_UNDER_MAINTENANCE) };
  }

  if (error.code === ErrorCode.DEVICE_NOT_REGISTERED) {
    return { form: getErrorMessage(ErrorCode.DEVICE_NOT_REGISTERED) };
  }

  return { form: message || 'Registration failed. Please try again.' };
}

export const REGISTRATION_FIELD_ORDER: RegistrationField[] = [
  'username',
  'phoneNumber',
  'password',
  'confirmPassword',
  'firstName',
  'lastName',
];

export function firstRegistrationFieldWithError(
  errors: RegistrationFormErrors,
): RegistrationField | undefined {
  return REGISTRATION_FIELD_ORDER.find((field) => Boolean(errors[field]));
}
