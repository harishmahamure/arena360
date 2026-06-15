import { isUserRole, type UserRole } from '@gaming-cafe/contracts';
import {
  phoneDigitsSchema,
  trimmedOptionalString,
  trimmedString,
  usernameSchema,
  validationMessages,
} from '@gaming-cafe/utils';
import * as yup from 'yup';

export const userRoleOptions: { value: UserRole; label: string }[] = [
  { value: 'player', label: 'Player' },
  { value: 'staff', label: 'Staff' },
  { value: 'admin', label: 'Admin' },
];

export const adminCreateRoleOptions: { value: UserRole; label: string }[] = [
  { value: 'player', label: 'Player' },
  { value: 'staff', label: 'Staff' },
];

export const isActiveOptions = [
  { value: true, label: 'Active' },
  { value: false, label: 'Inactive' },
];

export const createPlayerSchema = yup.object({
  username: usernameSchema,

  phoneNumber: phoneDigitsSchema('Phone Number'),

  password: yup
    .string()
    .transform((value) => (value == null ? '' : String(value).trim()))
    .min(8, 'Password must be at least 8 characters')
    .required(validationMessages.required('Password')),

  confirmPassword: yup
    .string()
    .transform((value) => (value == null ? '' : String(value).trim()))
    .oneOf([yup.ref('password')], 'Passwords must match')
    .required(validationMessages.required('Confirm Password')),

  firstName: trimmedOptionalString().max(50, 'First name must not exceed 50 characters'),

  lastName: trimmedOptionalString().max(50, 'Last name must not exceed 50 characters'),

  role: yup.string().oneOf(['player', 'staff'], 'Please select a valid role').default('player'),
});

export type CreatePlayerFormData = yup.InferType<typeof createPlayerSchema>;

export const createPlayerDefaultValues: CreatePlayerFormData = {
  username: '',
  phoneNumber: '',
  password: '',
  confirmPassword: '',
  firstName: '',
  lastName: '',
  role: 'player',
};

export const updatePlayerSchema = yup.object({
  username: usernameSchema,

  phoneNumber: phoneDigitsSchema('Phone Number'),

  firstName: trimmedOptionalString().max(50, 'First name must not exceed 50 characters'),

  lastName: trimmedOptionalString().max(50, 'Last name must not exceed 50 characters'),

  role: yup
    .string()
    .test('valid-role', 'Please select a valid role', (value) => (value ? isUserRole(value) : true))
    .optional(),

  isActive: yup.boolean().optional(),
});

export type UpdatePlayerFormData = yup.InferType<typeof updatePlayerSchema>;
