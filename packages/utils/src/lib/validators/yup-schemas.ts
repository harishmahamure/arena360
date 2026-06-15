import * as yup from 'yup';
import { digitsOnly, normalizeUsername, trimOptional, trimValue } from '../helpers/string-utils';

// ==============================================================
// VALIDATION MESSAGES
// ==============================================================

export const validationMessages = {
  required: (field: string) => `${field} is required`,
  email: 'Please enter a valid email address',
  min: (field: string, min: number) => `${field} must be at least ${min} characters`,
  max: (field: string, max: number) => `${field} must be at most ${max} characters`,
  minValue: (field: string, min: number) => `${field} must be at least ${min}`,
  maxValue: (field: string, max: number) => `${field} must be at most ${max}`,
  url: 'Please enter a valid URL',
  phone: 'Please enter a valid phone number',
  password:
    'Password must contain at least 8 characters, one uppercase, one lowercase, and one number',
  passwordMatch: 'Passwords do not match',
  numeric: 'Please enter a valid number',
  alphanumeric: 'Please use only letters and numbers',
  date: 'Please enter a valid date',
  future: 'Date must be in the future',
  past: 'Date must be in the past',
  pan: 'Please enter a valid PAN number (e.g., ABCDE1234F)',
  aadhar: 'Please enter a valid Aadhar number (12 digits)',
  gstin: 'Please enter a valid GSTIN (e.g., 22AAAAA0000A1Z5)',
  ifsc: 'Please enter a valid IFSC code (e.g., SBIN0001234)',
  pincode: 'Please enter a valid 6-digit pincode',
  alphabetic: 'Please use only letters',
  noSpecialChars: 'Special characters are not allowed',
};

// ==============================================================
// COMMON STRING VALIDATORS
// ==============================================================

/**
 * Required string validation schema
 */
export const requiredString = (fieldName = 'Field') =>
  yup.string().required(validationMessages.required(fieldName));

/**
 * Optional string validation schema
 */
export const optionalString = () =>
  yup
    .string()
    .transform((value) => trimOptional(value))
    .optional();

/**
 * String with min/max length validation
 */
export const stringWithLength = (
  fieldName = 'Field',
  min?: number,
  max?: number,
  required = true,
) => {
  let schema = yup.string().transform((value) => trimValue(value));

  if (min) {
    schema = schema.min(min, validationMessages.min(fieldName, min));
  }

  if (max) {
    schema = schema.max(max, validationMessages.max(fieldName, max));
  }

  return required ? schema.required(validationMessages.required(fieldName)) : schema.optional();
};

/**
 * Alphabetic only validation schema
 */
export const alphabeticSchema = (fieldName = 'Field', required = true) => {
  const schema = yup.string().matches(/^[a-zA-Z\s]+$/, validationMessages.alphabetic);

  return required ? schema.required(validationMessages.required(fieldName)) : schema.optional();
};

/**
 * Alphanumeric validation schema
 */
export const alphanumericSchema = (fieldName = 'Field', required = true) => {
  const schema = yup.string().matches(/^[a-zA-Z0-9]+$/, validationMessages.alphanumeric);

  return required ? schema.required(validationMessages.required(fieldName)) : schema.optional();
};

/**
 * No special characters validation schema
 */
export const noSpecialCharsSchema = (fieldName = 'Field', required = true) => {
  const schema = yup.string().matches(/^[a-zA-Z0-9\s]+$/, validationMessages.noSpecialChars);

  return required ? schema.required(validationMessages.required(fieldName)) : schema.optional();
};

// ==============================================================
// EMAIL & COMMUNICATION VALIDATORS
// ==============================================================

/**
 * Email validation schema
 */
export const emailSchema = yup
  .string()
  .email(validationMessages.email)
  .lowercase()
  .trim()
  .required(validationMessages.required('Email'));

/**
 * Optional email validation schema
 */
export const optionalEmailSchema = yup
  .string()
  .email(validationMessages.email)
  .lowercase()
  .trim()
  .optional();

// ==============================================================
// PASSWORD & SECURITY VALIDATORS
// ==============================================================

/**
 * Password validation schema
 */
export const passwordSchema = yup
  .string()
  .min(8, validationMessages.min('Password', 8))
  .matches(/^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)/, validationMessages.password)
  .required(validationMessages.required('Password'));

/**
 * Strong password validation schema (includes special characters)
 */
export const strongPasswordSchema = yup
  .string()
  .min(8, validationMessages.min('Password', 8))
  .matches(
    /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])/,
    'Password must contain at least 8 characters, one uppercase, one lowercase, one number, and one special character',
  )
  .required(validationMessages.required('Password'));

/**
 * Confirm password validation schema
 */
export const confirmPasswordSchema = (passwordField = 'password') =>
  yup
    .string()
    .oneOf([yup.ref(passwordField)], validationMessages.passwordMatch)
    .required(validationMessages.required('Confirm Password'));

// ==============================================================
// URL & WEB VALIDATORS
// ==============================================================

/**
 * Phone number validation schema (International format)
 */
export const phoneSchema = yup
  .string()
  .matches(/^[+]?[(]?[0-9]{3}[)]?[-\s.]?[0-9]{3}[-\s.]?[0-9]{4,6}$/, validationMessages.phone)
  .required(validationMessages.required('Phone number'));

/**
 * Indian mobile number validation schema (10 digits)
 */
export const indianMobileSchema = yup
  .string()
  .matches(
    /^[6-9]\d{9}$/,
    'Please enter a valid Indian mobile number (10 digits starting with 6-9)',
  )
  .required(validationMessages.required('Mobile number'));

/**
 * Optional phone validation schema
 */
export const optionalPhoneSchema = yup
  .string()
  .matches(/^[+]?[(]?[0-9]{3}[)]?[-\s.]?[0-9]{3}[-\s.]?[0-9]{4,6}$/, validationMessages.phone)
  .optional();

// ==============================================================
// INDIAN DOCUMENT VALIDATORS
// ==============================================================

/**
 * PAN (Permanent Account Number) validation schema
 * Format: ABCDE1234F (5 letters, 4 digits, 1 letter)
 */
export const panSchema = yup
  .string()
  .matches(/^[A-Z]{5}[0-9]{4}[A-Z]{1}$/, validationMessages.pan)
  .uppercase()
  .trim()
  .required(validationMessages.required('PAN'));

/**
 * Optional PAN validation schema
 */
export const optionalPanSchema = yup
  .string()
  .matches(/^[A-Z]{5}[0-9]{4}[A-Z]{1}$/, validationMessages.pan)
  .uppercase()
  .trim()
  .optional();

/**
 * Aadhar number validation schema (12 digits)
 */
export const aadharSchema = yup
  .string()
  .matches(/^\d{12}$/, validationMessages.aadhar)
  .test('valid-aadhar', validationMessages.aadhar, (value) => {
    if (!value) return false;
    // Aadhar cannot start with 0 or 1
    return !value.startsWith('0') && !value.startsWith('1');
  })
  .required(validationMessages.required('Aadhar'));

/**
 * Optional Aadhar validation schema
 */
export const optionalAadharSchema = yup
  .string()
  .matches(/^\d{12}$/, validationMessages.aadhar)
  .test('valid-aadhar', validationMessages.aadhar, (value) => {
    if (!value) return true;
    return !value.startsWith('0') && !value.startsWith('1');
  })
  .optional();

/**
 * GSTIN (Goods and Services Tax Identification Number) validation schema
 * Format: 22AAAAA0000A1Z5 (15 characters)
 */
export const gstinSchema = yup
  .string()
  .matches(/^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z]{1}[1-9A-Z]{1}Z[0-9A-Z]{1}$/, validationMessages.gstin)
  .uppercase()
  .trim()
  .required(validationMessages.required('GSTIN'));

/**
 * Optional GSTIN validation schema
 */
export const optionalGstinSchema = yup
  .string()
  .matches(/^[0-9]{2}[A-Z]{5}[0-9]{4}[A-Z]{1}[1-9A-Z]{1}Z[0-9A-Z]{1}$/, validationMessages.gstin)
  .uppercase()
  .trim()
  .optional();

/**
 * IFSC (Indian Financial System Code) validation schema
 * Format: SBIN0001234 (4 letters, 0, 6 alphanumeric)
 */
export const ifscSchema = yup
  .string()
  .matches(/^[A-Z]{4}0[A-Z0-9]{6}$/, validationMessages.ifsc)
  .uppercase()
  .trim()
  .required(validationMessages.required('IFSC Code'));

/**
 * Optional IFSC validation schema
 */
export const optionalIfscSchema = yup
  .string()
  .matches(/^[A-Z]{4}0[A-Z0-9]{6}$/, validationMessages.ifsc)
  .uppercase()
  .trim()
  .optional();

/**
 * Indian Pincode validation schema (6 digits)
 */
export const pincodeSchema = yup
  .string()
  .matches(/^[1-9][0-9]{5}$/, validationMessages.pincode)
  .required(validationMessages.required('Pincode'));

/**
 * Optional Pincode validation schema
 */
export const optionalPincodeSchema = yup
  .string()
  .matches(/^[1-9][0-9]{5}$/, validationMessages.pincode)
  .optional();

// ==============================================================
// URL & WEB VALIDATORS
// ==============================================================

/**
 * URL validation schema
 */
export const urlSchema = yup
  .string()
  .url(validationMessages.url)
  .required(validationMessages.required('URL'));

/**
 * Optional URL validation schema
 */
export const optionalUrlSchema = yup.string().url(validationMessages.url).optional();

// ==============================================================
// NAME & TEXT VALIDATORS
// ==============================================================

/**
 * Name validation schema
 */
export const nameSchema = (fieldName = 'Name') =>
  yup
    .string()
    .min(2, validationMessages.min(fieldName, 2))
    .max(50, validationMessages.max(fieldName, 50))
    .matches(/^[a-zA-Z\s]+$/, `${fieldName} should only contain letters`)
    .trim()
    .required(validationMessages.required(fieldName));

/**
 * Optional name validation schema
 */
export const optionalNameSchema = (fieldName = 'Name') =>
  yup
    .string()
    .min(2, validationMessages.min(fieldName, 2))
    .max(50, validationMessages.max(fieldName, 50))
    .matches(/^[a-zA-Z\s]+$/, `${fieldName} should only contain letters`)
    .trim()
    .optional();

/**
 * Username validation schema
 */
export const USERNAME_HELPER_TEXT =
  '3–50 characters; spaces become underscores; letters, numbers, dots, hyphens, and underscores only';

export const usernameSchema = yup
  .string()
  .transform((value) => normalizeUsername(value))
  .min(3, validationMessages.min('Username', 3))
  .max(50, validationMessages.max('Username', 50))
  .matches(
    /^[a-zA-Z0-9._-]+$/,
    'Username can only contain letters, numbers, dots, hyphens, and underscores',
  )
  .test('no-whitespace', 'Username cannot contain spaces', (value) =>
    value ? !/\s/.test(value) : true,
  )
  .required(validationMessages.required('Username'));

/**
 * Trim required string fields on submit.
 */
export const trimmedString = (fieldName = 'Field') =>
  yup
    .string()
    .transform((value) => trimValue(value))
    .required(validationMessages.required(fieldName));

/**
 * Trim optional string fields; empty becomes undefined.
 */
export const trimmedOptionalString = () =>
  yup
    .string()
    .transform((value) => trimOptional(value))
    .optional()
    .nullable();

/**
 * Phone number: trim, digits only, min 10.
 */
export const phoneDigitsSchema = (fieldName = 'Phone Number') =>
  yup
    .string()
    .transform((value) => digitsOnly(trimValue(value)))
    .min(10, `${fieldName} must be at least 10 digits`)
    .required(validationMessages.required(fieldName));

/**
 * Slug validation schema (for URLs)
 */
export const slugSchema = yup
  .string()
  .matches(
    /^[a-z0-9]+(?:-[a-z0-9]+)*$/,
    'Slug must be lowercase letters, numbers, and hyphens only',
  )
  .trim()
  .required(validationMessages.required('Slug'));

// ==============================================================
// NUMERIC VALIDATORS
// ==============================================================

/**
 * Numeric validation schema
 */
export const numericSchema = (fieldName = 'Value') =>
  yup
    .number()
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Optional numeric validation schema
 */
export const optionalNumericSchema = () =>
  yup.number().typeError(validationMessages.numeric).optional();

/**
 * Positive number validation schema
 */
export const positiveNumberSchema = (fieldName = 'Value') =>
  yup
    .number()
    .positive(`${fieldName} must be positive`)
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Non-negative number validation schema (includes zero)
 */
export const nonNegativeNumberSchema = (fieldName = 'Value') =>
  yup
    .number()
    .min(0, `${fieldName} cannot be negative`)
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Integer validation schema
 */
export const integerSchema = (fieldName = 'Value') =>
  yup
    .number()
    .integer(`${fieldName} must be a whole number`)
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Positive integer validation schema
 */
export const positiveIntegerSchema = (fieldName = 'Value') =>
  yup
    .number()
    .integer(`${fieldName} must be a whole number`)
    .positive(`${fieldName} must be positive`)
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Number range validation schema
 */
export const numberRangeSchema = (fieldName = 'Value', min?: number, max?: number) => {
  let schema = yup.number().typeError(validationMessages.numeric);

  if (min !== undefined) {
    schema = schema.min(min, validationMessages.minValue(fieldName, min));
  }

  if (max !== undefined) {
    schema = schema.max(max, validationMessages.maxValue(fieldName, max));
  }

  return schema.required(validationMessages.required(fieldName));
};

/**
 * Percentage validation schema (0-100)
 */
export const percentageSchema = (fieldName = 'Percentage') =>
  yup
    .number()
    .min(0, `${fieldName} must be between 0 and 100`)
    .max(100, `${fieldName} must be between 0 and 100`)
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

/**
 * Currency validation schema (positive with 2 decimal places)
 */
export const currencySchema = (fieldName = 'Amount') =>
  yup
    .number()
    .positive(`${fieldName} must be positive`)
    .test('decimal-places', `${fieldName} should have at most 2 decimal places`, (value) => {
      if (value === undefined || value === null) return true;
      return /^\d+(\.\d{1,2})?$/.test(value.toString());
    })
    .typeError(validationMessages.numeric)
    .required(validationMessages.required(fieldName));

// ==============================================================
// DATE & TIME VALIDATORS
// ==============================================================

/**
 * Date validation schema
 */
export const dateSchema = (fieldName = 'Date') =>
  yup.date().typeError(validationMessages.date).required(validationMessages.required(fieldName));

/**
 * Optional date validation schema
 */
export const optionalDateSchema = () => yup.date().typeError(validationMessages.date).optional();

/**
 * Future date validation schema
 */
export const futureDateSchema = (fieldName = 'Date') =>
  yup
    .date()
    .min(new Date(), validationMessages.future)
    .typeError(validationMessages.date)
    .required(validationMessages.required(fieldName));

/**
 * Past date validation schema
 */
export const pastDateSchema = (fieldName = 'Date') =>
  yup
    .date()
    .max(new Date(), validationMessages.past)
    .typeError(validationMessages.date)
    .required(validationMessages.required(fieldName));

/**
 * Date range validation schema
 */
export const dateRangeSchema = (fieldName = 'Date', minDate?: Date, maxDate?: Date) => {
  let schema = yup.date().typeError(validationMessages.date);

  if (minDate) {
    schema = schema.min(minDate, `${fieldName} must be after ${minDate.toLocaleDateString()}`);
  }

  if (maxDate) {
    schema = schema.max(maxDate, `${fieldName} must be before ${maxDate.toLocaleDateString()}`);
  }

  return schema.required(validationMessages.required(fieldName));
};

/**
 * Age validation schema (for date of birth)
 */
export const ageSchema = (minAge = 18, maxAge = 120) => {
  const today = new Date();
  const maxDate = new Date(today.getFullYear() - minAge, today.getMonth(), today.getDate());
  const minDate = new Date(today.getFullYear() - maxAge, today.getMonth(), today.getDate());

  return yup
    .date()
    .max(maxDate, `You must be at least ${minAge} years old`)
    .min(minDate, `Age cannot exceed ${maxAge} years`)
    .typeError(validationMessages.date)
    .required(validationMessages.required('Date of Birth'));
};

// ==============================================================
// FILE & MEDIA VALIDATORS
// ==============================================================

/**
 * File validation schema
 */
export const fileSchema = (
  maxSize: number = 5 * 1024 * 1024, // 5MB
  allowedTypes: string[] = ['image/jpeg', 'image/png', 'image/gif'],
) =>
  yup
    .mixed<File>()
    .test('fileSize', `File size must be less than ${maxSize / 1024 / 1024}MB`, (value) => {
      if (!value) return true;
      return value instanceof File && value.size <= maxSize;
    })
    .test('fileType', 'Unsupported file type', (value) => {
      if (!value) return true;
      return value instanceof File && allowedTypes.includes(value.type);
    })
    .required(validationMessages.required('File'));

/**
 * Optional file validation schema
 */
export const optionalFileSchema = (
  maxSize: number = 5 * 1024 * 1024,
  allowedTypes: string[] = ['image/jpeg', 'image/png', 'image/gif'],
) =>
  yup
    .mixed<File>()
    .test('fileSize', `File size must be less than ${maxSize / 1024 / 1024}MB`, (value) => {
      if (!value) return true;
      return value instanceof File && value.size <= maxSize;
    })
    .test('fileType', 'Unsupported file type', (value) => {
      if (!value) return true;
      return value instanceof File && allowedTypes.includes(value.type);
    })
    .nullable();

/**
 * Image file validation schema
 */
export const imageFileSchema = (maxSize: number = 5 * 1024 * 1024) =>
  fileSchema(maxSize, ['image/jpeg', 'image/png', 'image/gif', 'image/webp']);

/**
 * Document file validation schema
 */
export const documentFileSchema = (maxSize: number = 10 * 1024 * 1024) =>
  fileSchema(maxSize, [
    'application/pdf',
    'application/msword',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  ]);

// ==============================================================
// ARRAY & COLLECTION VALIDATORS
// ==============================================================

/**
 * Array validation schema
 */
export const arraySchema = <T>(itemSchema: yup.Schema<T>, min = 1) =>
  yup
    .array()
    .of(itemSchema)
    .min(min, `At least ${min} item(s) required`)
    .required(validationMessages.required('Items'));

/**
 * Optional array validation schema
 */
export const optionalArraySchema = <T>(itemSchema: yup.Schema<T>) =>
  yup.array().of(itemSchema).optional();

/**
 * Non-empty array validation schema
 */
export const nonEmptyArraySchema = <T>(itemSchema: yup.Schema<T>) =>
  yup
    .array()
    .of(itemSchema)
    .min(1, 'At least one item is required')
    .required(validationMessages.required('Items'));

/**
 * String array validation schema
 */
export const stringArraySchema = (min = 1) => arraySchema(yup.string().required(), min);

// ==============================================================
// BOOLEAN VALIDATORS
// ==============================================================

/**
 * Required boolean validation schema
 */
export const booleanSchema = (fieldName = 'Field') =>
  yup.boolean().required(validationMessages.required(fieldName));

/**
 * Required true validation schema (for checkboxes like terms acceptance)
 */
export const requiredTrueSchema = (message = 'This field must be checked') =>
  yup.boolean().oneOf([true], message).required();

// ==============================================================
// FORM SCHEMAS
// ==============================================================

/**
 * Login form schema
 */
export const loginSchema = yup.object({
  email: emailSchema,
  password: yup.string().required(validationMessages.required('Password')),
  rememberMe: yup.boolean(),
});

/**
 * Register form schema
 */
export const registerSchema = yup.object({
  firstName: nameSchema('First name'),
  lastName: nameSchema('Last name'),
  email: emailSchema,
  password: passwordSchema,
  confirmPassword: confirmPasswordSchema(),
  acceptTerms: yup.boolean().oneOf([true], 'You must accept the terms and conditions').required(),
});

/**
 * Profile update schema
 */
export const profileSchema = yup.object({
  firstName: nameSchema('First name'),
  lastName: nameSchema('Last name'),
  email: emailSchema,
  phone: phoneSchema.optional(),
  bio: yup.string().max(500, validationMessages.max('Bio', 500)).optional(),
});

/**
 * Change password schema
 */
export const changePasswordSchema = yup.object({
  currentPassword: yup.string().required(validationMessages.required('Current password')),
  newPassword: passwordSchema,
  confirmNewPassword: confirmPasswordSchema('newPassword'),
});

/**
 * Forgot password schema
 */
export const forgotPasswordSchema = yup.object({
  email: emailSchema,
});

/**
 * Reset password schema
 */
export const resetPasswordSchema = yup.object({
  password: passwordSchema,
  confirmPassword: confirmPasswordSchema(),
});

/**
 * Contact form schema
 */
export const contactSchema = yup.object({
  name: nameSchema(),
  email: emailSchema,
  subject: yup.string().required(validationMessages.required('Subject')),
  message: yup
    .string()
    .min(10, validationMessages.min('Message', 10))
    .max(1000, validationMessages.max('Message', 1000))
    .required(validationMessages.required('Message')),
});

// ==============================================================
// TYPE EXPORTS
// ==============================================================

export type LoginFormData = yup.InferType<typeof loginSchema>;
export type RegisterFormData = yup.InferType<typeof registerSchema>;
export type ProfileFormData = yup.InferType<typeof profileSchema>;
export type ChangePasswordFormData = yup.InferType<typeof changePasswordSchema>;
export type ForgotPasswordFormData = yup.InferType<typeof forgotPasswordSchema>;
export type ResetPasswordFormData = yup.InferType<typeof resetPasswordSchema>;
export type ContactFormData = yup.InferType<typeof contactSchema>;
