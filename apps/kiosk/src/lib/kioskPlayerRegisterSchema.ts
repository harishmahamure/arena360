import {
  phoneDigitsSchema,
  trimmedOptionalString,
  usernameSchema,
  validationMessages,
} from '@gaming-cafe/utils';
import * as yup from 'yup';

/** Client validation for kiosk self-registration (DRAFT-0049). */
export const kioskPlayerRegisterSchema = yup.object({
  username: usernameSchema,
  phoneNumber: phoneDigitsSchema('Phone number'),
  password: yup
    .string()
    .transform((value) => (value == null ? '' : String(value).trim()))
    .min(8, 'Password must be at least 8 characters')
    .required(validationMessages.required('Password')),
  confirmPassword: yup
    .string()
    .transform((value) => (value == null ? '' : String(value).trim()))
    .oneOf([yup.ref('password')], 'Passwords must match')
    .required(validationMessages.required('Confirm password')),
  firstName: trimmedOptionalString().max(50, 'First name must not exceed 50 characters'),
  lastName: trimmedOptionalString().max(50, 'Last name must not exceed 50 characters'),
});

export type KioskPlayerRegisterFormData = yup.InferType<typeof kioskPlayerRegisterSchema>;
