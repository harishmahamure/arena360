import { optionalString, stringWithLength, validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export enum DeviceType {
  PC = 'PC',
  CONSOLE = 'CONSOLE',
  PS5 = 'PS5',
  PS4 = 'PS4',
}

export enum DeviceSubType {
  HIGH_END_PCS = 'HIGH_END_PCS',
  MID_RANGE_PCS = 'MID_RANGE_PCS',
  PREMIUM_TV_CONSOLES = 'PREMIUM_TV_CONSOLES',
  STANDARD_TV_CONSOLES = 'STANDARD_TV_CONSOLES',
}

export const deviceTypeOptions = [
  { value: DeviceType.PC, label: 'PC' },
  { value: DeviceType.CONSOLE, label: 'Console' },
  { value: DeviceType.PS5, label: 'PS5' },
  { value: DeviceType.PS4, label: 'PS4' },
];

export const deviceSubTypeOptions = [
  { value: DeviceSubType.HIGH_END_PCS, label: 'High End PCs' },
  { value: DeviceSubType.MID_RANGE_PCS, label: 'Mid Range PCs' },
  { value: DeviceSubType.PREMIUM_TV_CONSOLES, label: 'Premium TV Consoles' },
  { value: DeviceSubType.STANDARD_TV_CONSOLES, label: 'Standard TV Consoles' },
];

export const PlanTypeValues = {
  TIME_BASED: 'time_based',
  WEEKEND_SPECIAL: 'weekend_special',
} as const;

export type PlanTypeType = (typeof PlanTypeValues)[keyof typeof PlanTypeValues];

export const planTypeOptions = [
  { value: PlanTypeValues.TIME_BASED, label: 'Time Plan' },
  { value: PlanTypeValues.WEEKEND_SPECIAL, label: 'Happy Hours' },
];

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
    .oneOf(Object.values(PlanTypeValues), 'Please select a valid plan type')
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
      is: PlanTypeValues.WEEKEND_SPECIAL,
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
      is: PlanTypeValues.WEEKEND_SPECIAL,
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
    .oneOf(Object.values(DeviceType), 'Please select a valid device type')
    .optional()
    .nullable(),

  deviceSubType: yup
    .string()
    .oneOf(Object.values(DeviceSubType), 'Please select a valid device sub type')
    .optional()
    .nullable(),

  allowedDays: yup
    .array()
    .of(yup.string().oneOf(WEEKDAY_OPTIONS.map((d) => d.value)).required())
    .optional()
    .nullable(),

  allowedMonths: yup
    .array()
    .of(yup.number().min(1).max(12).required())
    .optional()
    .nullable(),
});

export type CreatePlanFormData = yup.InferType<typeof createPlanSchema>;

export const createPlanDefaultValues: CreatePlanFormData = {
  name: '',
  description: '',
  price: 0,
  planType: PlanTypeValues.TIME_BASED,
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
