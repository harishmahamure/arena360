import {
  DEVICE_SUB_TYPE_VALUES,
  DEVICE_TYPE_VALUES,
  PLAN_TYPE_ADMIN_VALUES,
  PlanType,
} from '@gaming-cafe/contracts';
import { optionalString, stringWithLength, validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export {
  DeviceSubType,
  DeviceType,
  deviceSubTypeOptions,
  deviceTypeOptions,
  PlanType,
  planTypeOptions,
} from '@gaming-cafe/contracts';

export const PlanTypeValues = PlanType;
export type PlanTypeType = (typeof PlanType)[keyof typeof PlanType];

export const WEEKDAY_OPTIONS = [
  { value: 'monday', label: 'Monday' },
  { value: 'tuesday', label: 'Tuesday' },
  { value: 'wednesday', label: 'Wednesday' },
  { value: 'thursday', label: 'Thursday' },
  { value: 'friday', label: 'Friday' },
  { value: 'saturday', label: 'Saturday' },
  { value: 'sunday', label: 'Sunday' },
];

export const MONTH_OPTIONS = [
  { value: 1, label: 'January' },
  { value: 2, label: 'February' },
  { value: 3, label: 'March' },
  { value: 4, label: 'April' },
  { value: 5, label: 'May' },
  { value: 6, label: 'June' },
  { value: 7, label: 'July' },
  { value: 8, label: 'August' },
  { value: 9, label: 'September' },
  { value: 10, label: 'October' },
  { value: 11, label: 'November' },
  { value: 12, label: 'December' },
];

export const createPlanSchema = yup.object({
  name: stringWithLength('Plan name', undefined, 100, true),

  description: optionalString(),

  price: yup
    .number()
    .min(0.01, 'Price must be greater than 0')
    .required(validationMessages.required('Price')),

  planType: yup
    .string()
    .oneOf([...PLAN_TYPE_ADMIN_VALUES], 'Please select a valid plan type')
    .required(validationMessages.required('Plan Type')),

  validityDays: yup
    .number()
    .positive('Validity days must be greater than 0')
    .integer('Validity days must be a whole number')
    .required(validationMessages.required('Validity days'))
    .default(7),

  timeWindowStart: yup
    .string()
    .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]$/, {
      message: 'Time must be in HH:MM:SS format',
      excludeEmptyString: true,
    })
    .optional()
    .nullable()
    .when('planType', {
      is: PlanType.WEEKEND_SPECIAL,
      then: (schema) => schema.required('Time window start is required for Happy Hours'),
    }),

  timeWindowEnd: yup
    .string()
    .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]$/, {
      message: 'Time must be in HH:MM:SS format',
      excludeEmptyString: true,
    })
    .optional()
    .nullable()
    .when('planType', {
      is: PlanType.WEEKEND_SPECIAL,
      then: (schema) => schema.required('Time window end is required for Happy Hours'),
    })
    .test('time-window-order', 'End time must be after start time', function (value) {
      const { timeWindowStart } = this.parent;
      if (!timeWindowStart || !value) return true;
      return timeWindowStart < value;
    }),

  timeCredits: yup
    .number()
    .positive('Time credits must be greater than 0')
    .integer('Time credits must be a whole number')
    .required(validationMessages.required('Time credits')),

  isActive: yup.boolean().optional().default(true),

  deviceType: yup
    .string()
    .oneOf([...DEVICE_TYPE_VALUES], 'Please select a valid device type')
    .optional()
    .nullable(),

  deviceSubType: yup
    .string()
    .oneOf([...DEVICE_SUB_TYPE_VALUES], 'Please select a valid device sub type')
    .optional()
    .nullable(),

  allowedDays: yup
    .array()
    .of(
      yup
        .string()
        .oneOf(WEEKDAY_OPTIONS.map((d) => d.value))
        .required(),
    )
    .optional()
    .nullable(),

  allowedMonths: yup.array().of(yup.number().min(1).max(12).required()).optional().nullable(),
});

export type CreatePlanFormData = yup.InferType<typeof createPlanSchema>;

export const createPlanDefaultValues: CreatePlanFormData = {
  name: '',
  description: '',
  price: 0,
  planType: PlanType.TIME_BASED,
  validityDays: 7,
  timeWindowStart: undefined,
  timeWindowEnd: undefined,
  timeCredits: 300,
  isActive: true,
  deviceType: undefined,
  deviceSubType: undefined,
  allowedDays: undefined,
  allowedMonths: undefined,
};
