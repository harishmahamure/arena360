import { isUserRole, type UserRole } from '@gaming-cafe/contracts';
import { validationMessages } from '@gaming-cafe/utils';
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
  username: yup
    .string()
    .min(3, 'Username must be at least 3 characters')
    .max(50, 'Username must not exceed 50 characters')
    .required(validationMessages.required('Username')),

  phoneNumber: yup
    .string()
    .min(10, 'Phone number must be at least 10 digits')
    .required(validationMessages.required('Phone Number')),

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
  username: yup
    .string()
    .min(3, 'Username must be at least 3 characters')
    .max(50, 'Username must not exceed 50 characters')
    .required(validationMessages.required('Username')),

  phoneNumber: yup
    .string()
    .min(10, 'Phone number must be at least 10 digits')
    .required(validationMessages.required('Phone Number')),

  firstName: yup.string().max(50, 'First name must not exceed 50 characters').optional().nullable(),

  lastName: yup.string().max(50, 'Last name must not exceed 50 characters').optional().nullable(),

  role: yup
    .string()
    .test('valid-role', 'Please select a valid role', (value) => (value ? isUserRole(value) : true))
    .optional(),

  isActive: yup.boolean().optional(),
});

export type UpdatePlayerFormData = yup.InferType<typeof updatePlayerSchema>;
