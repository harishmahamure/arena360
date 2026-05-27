import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

// Enum values
export const UserRoleValues = {
  ADMIN: 'admin',
  PLAYER: 'player',
} as const;

export type UserRoleType = (typeof UserRoleValues)[keyof typeof UserRoleValues];

// Options for select fields
export const userRoleOptions = [
  { value: UserRoleValues.ADMIN, label: 'Admin' },
  { value: UserRoleValues.PLAYER, label: 'Player' },
];

export const isActiveOptions = [
  { value: true, label: 'Active' },
  { value: false, label: 'Inactive' },
];

export const createPlayerSchema = yup.object({
  username: yup
    .string()
    .min(3, 'Username must be at least 3 characters')
    .max(50, 'Username must not exceed 50 characters')
    .required(validationMessages.required('Username')),

  email: yup.string().email(validationMessages.email).optional().nullable(),

  password: yup
    .string()
    .min(8, 'Password must be at least 8 characters')
    .required(validationMessages.required('Password')),

  confirmPassword: yup
    .string()
    .oneOf([yup.ref('password')], 'Passwords must match')
    .required(validationMessages.required('Confirm Password')),

  firstName: yup.string().max(50, 'First name must not exceed 50 characters').optional().nullable(),

  lastName: yup.string().max(50, 'Last name must not exceed 50 characters').optional().nullable(),
});

export type CreatePlayerFormData = yup.InferType<typeof createPlayerSchema>;

export const createPlayerDefaultValues: CreatePlayerFormData = {
  username: '',
  password: '',
  confirmPassword: '',
  firstName: '',
  lastName: '',
  email: '',
};

// Update player schema (no password fields)
export const updatePlayerSchema = yup.object({
  email: yup.string().email('Please enter a valid email address').optional().nullable(),

  username: yup
    .string()
    .min(3, 'Username must be at least 3 characters')
    .max(50, 'Username must not exceed 50 characters')
    .required(validationMessages.required('Username')),

  firstName: yup.string().max(50, 'First name must not exceed 50 characters').optional().nullable(),

  lastName: yup.string().max(50, 'Last name must not exceed 50 characters').optional().nullable(),

  role: yup.string().oneOf(Object.values(UserRoleValues), 'Please select a valid role').optional(),

  isActive: yup.boolean().optional(),
});

export type UpdatePlayerFormData = yup.InferType<typeof updatePlayerSchema>;
