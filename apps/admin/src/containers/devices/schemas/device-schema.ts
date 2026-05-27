import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

// Enum values
export const DeviceStatusValues = {
  OPERATIONAL: 'operational',
  UNDER_MAINTENANCE: 'under_maintenance',
  OUT_OF_ORDER: 'out_of_service',
  IN_USE: 'in_use',
  AVAILABLE: 'available',
} as const;

export type DeviceStatusType = (typeof DeviceStatusValues)[keyof typeof DeviceStatusValues];

// Options for select fields
export const deviceStatusOptions = [
  { value: DeviceStatusValues.OPERATIONAL, label: 'Operational' },
  { value: DeviceStatusValues.UNDER_MAINTENANCE, label: 'Under Maintenance' },
  { value: DeviceStatusValues.OUT_OF_ORDER, label: 'Out of Service' },
  { value: DeviceStatusValues.IN_USE, label: 'In Use' },
  { value: DeviceStatusValues.AVAILABLE, label: 'Available' },
];

// Common device types for game zones
export const deviceTypeOptions = [
  { value: 'PlayStation 5', label: 'PlayStation 5' },
  { value: 'PlayStation 4', label: 'PlayStation 4' },
  { value: 'Xbox Series X', label: 'Xbox Series X' },
  { value: 'Xbox Series S', label: 'Xbox Series S' },
  { value: 'Xbox One', label: 'Xbox One' },
  { value: 'Nintendo Switch', label: 'Nintendo Switch' },
  { value: 'Gaming PC', label: 'Gaming PC' },
  { value: 'VR Headset', label: 'VR Headset' },
  { value: 'Racing Simulator', label: 'Racing Simulator' },
  { value: 'Arcade Machine', label: 'Arcade Machine' },
  { value: 'Other', label: 'Other' },
];

// Create device schema
export const createDeviceSchema = yup.object({
  name: yup
    .string()
    .max(100, 'Name must not exceed 100 characters')
    .required(validationMessages.required('Device Name')),

  serialNumber: yup
    .string()
    .max(100, 'Serial number must not exceed 100 characters')
    .optional()
    .nullable(),

  localIpAddress: yup
    .string()
    .max(100, 'IP address must not exceed 100 characters')
    .matches(/^$|^(\d{1,3}\.){3}\d{1,3}$/, 'Please enter a valid IP address (e.g., 192.168.1.100)')
    .optional()
    .nullable(),

  deviceType: yup
    .string()
    .max(100, 'Device type must not exceed 100 characters')
    .required(validationMessages.required('Device Type')),

  location: yup.string().max(200, 'Location must not exceed 200 characters').optional().nullable(),

  status: yup
    .string()
    .oneOf(Object.values(DeviceStatusValues), 'Please select a valid status')
    .optional()
    .default(DeviceStatusValues.OPERATIONAL),
});

export type CreateDeviceFormData = yup.InferType<typeof createDeviceSchema>;

export const createDeviceDefaultValues: CreateDeviceFormData = {
  name: '',
  serialNumber: '',
  localIpAddress: '',
  deviceType: '',
  location: '',
  status: DeviceStatusValues.OPERATIONAL,
};

// Update device status schema
export const updateDeviceStatusSchema = yup.object({
  status: yup
    .string()
    .oneOf(Object.values(DeviceStatusValues), 'Please select a valid status')
    .required(validationMessages.required('Status')),
});

export type UpdateDeviceStatusFormData = yup.InferType<typeof updateDeviceStatusSchema>;
