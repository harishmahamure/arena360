import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const assignGameToDeviceSchema = yup.object({
  deviceId: yup
    .string()
    .uuid('Please select a valid device')
    .required(validationMessages.required('Device')),

  gameId: yup
    .string()
    .uuid('Please select a valid game')
    .required(validationMessages.required('Game')),

  installationDate: yup
    .date()
    .optional()
    .nullable()
    .transform((value, originalValue) => {
      if (originalValue === '' || originalValue === null) return null;
      return value;
    }),

  isActive: yup.boolean().optional().default(true),
});

export type AssignGameToDeviceFormData = yup.InferType<typeof assignGameToDeviceSchema>;

export const assignGameToDeviceDefaultValues: AssignGameToDeviceFormData = {
  deviceId: '',
  gameId: '',
  installationDate: null,
  isActive: true,
};
