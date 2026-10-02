import {
  DEVICE_SUB_TYPE_VALUES,
  DEVICE_TYPE_VALUES,
  DeviceStatus,
  deviceStatusOptions,
} from '@gaming-cafe/contracts';
import { trimmedOptionalString, trimmedString, validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export {
  DeviceSubType,
  DeviceType,
  deviceSubTypeOptions,
  deviceTypeOptions,
} from '@gaming-cafe/contracts';

export const DeviceStatusValues = DeviceStatus;
export type DeviceStatusType = (typeof DeviceStatus)[keyof typeof DeviceStatus];

export { deviceStatusOptions };

export const createDeviceSchema = yup.object({
  name: trimmedString('Device Name').max(100, 'Name must not exceed 100 characters'),

  serialNumber: trimmedOptionalString().max(100, 'Serial number must not exceed 100 characters'),

  localIpAddress: trimmedOptionalString()
    .max(100, 'IP address must not exceed 100 characters')
    .matches(/^$|^(\d{1,3}\.){3}\d{1,3}$/, 'Please enter a valid IP address (e.g., 192.168.1.100)'),

  deviceType: yup
    .string()
    .oneOf([...DEVICE_TYPE_VALUES], 'Please select a valid device type')
    .required(validationMessages.required('Device Type')),

  deviceSubType: yup
    .string()
    .oneOf([...DEVICE_SUB_TYPE_VALUES], 'Please select a valid device sub type')
    .required(validationMessages.required('Device Sub Type')),

  location: trimmedOptionalString().max(200, 'Location must not exceed 200 characters'),

  status: yup
    .string()
    .oneOf(Object.values(DeviceStatus), 'Please select a valid status')
    .optional()
    .default(DeviceStatus.OPERATIONAL),
});

export type CreateDeviceFormData = yup.InferType<typeof createDeviceSchema>;

export const createDeviceDefaultValues = {
  name: '',
  serialNumber: '',
  localIpAddress: '',
  deviceType: '',
  deviceSubType: '',
  location: '',
  status: DeviceStatus.OPERATIONAL,
} as unknown as CreateDeviceFormData;

export const updateDeviceStatusSchema = yup.object({
  status: yup
    .string()
    .oneOf(Object.values(DeviceStatus), 'Please select a valid status')
    .required(validationMessages.required('Status')),
});

export type UpdateDeviceStatusFormData = yup.InferType<typeof updateDeviceStatusSchema>;
