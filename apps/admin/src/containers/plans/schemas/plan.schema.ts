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
  SESSION_BASED: 'session_based',
  UNLIMITED_DAILY: 'unlimited_daily',
  HOURLY_RENTAL: 'hourly_rental',
  MONTHLY_SUBSCRIPTION: 'monthly_subscription',
  WEEKEND_SPECIAL: 'weekend_special',
} as const;

export type PlanTypeType = (typeof PlanTypeValues)[keyof typeof PlanTypeValues];

export const planTypeOptions = [
  { value: PlanTypeValues.TIME_BASED, label: 'Time Based' },
  { value: PlanTypeValues.SESSION_BASED, label: 'Session Based' },
  { value: PlanTypeValues.UNLIMITED_DAILY, label: 'Unlimited Daily' },
  { value: PlanTypeValues.HOURLY_RENTAL, label: 'Hourly Rental' },
  {
    value: PlanTypeValues.MONTHLY_SUBSCRIPTION,
    label: 'Monthly Subscription',
  },
  { value: PlanTypeValues.WEEKEND_SPECIAL, label: 'Weekend Special' },
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

  durationMinutes: yup
    .number()
    .positive('Duration must be greater than 0')
    .integer('Duration must be a whole number')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .when('planType', {
      is: PlanTypeValues.TIME_BASED,
      then: (schema) => schema.required('Duration is required for time-based plans'),
    }),

  validityDays: yup
    .number()
    .positive('Validity days must be greater than 0')
    .integer('Validity days must be a whole number')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .default(30)
    .when('planType', {
      is: (val: string) =>
        val === PlanTypeValues.UNLIMITED_DAILY || val === PlanTypeValues.MONTHLY_SUBSCRIPTION,
      then: (schema) => schema.required('Validity days is required for this plan type'),
    }),

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
      then: (schema) => schema.required('Time window start is required for weekend special'),
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
      then: (schema) => schema.required('Time window end is required for weekend special'),
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
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .when('planType', {
      is: PlanTypeValues.TIME_BASED,
      then: (schema) => schema.required('Time credits are required for time-based plans'),
    }),

  perMinuteRate: yup
    .number()
    .positive('Per minute rate must be greater than 0')
    .optional()
    .nullable()
    .default(1.0)
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .when('planType', {
      is: PlanTypeValues.HOURLY_RENTAL,
      then: (schema) => schema.required('Per minute rate is required for hourly rental'),
    }),

  maxSessions: yup
    .number()
    .positive('Max sessions must be greater than 0')
    .integer('Max sessions must be a whole number')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .when('planType', {
      is: PlanTypeValues.SESSION_BASED,
      then: (schema) => schema.required('Max sessions is required for session-based plans'),
    }),

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
});

export type CreatePlanFormData = yup.InferType<typeof createPlanSchema>;

export const createPlanDefaultValues: CreatePlanFormData = {
  name: '',
  description: '',
  price: 0,
  planType: PlanTypeValues.TIME_BASED,
  durationMinutes: undefined,
  validityDays: 30,
  timeWindowStart: undefined,
  timeWindowEnd: undefined,
  timeCredits: undefined,
  perMinuteRate: 1.0,
  maxSessions: undefined,
  isActive: true,
  deviceType: undefined,
  deviceSubType: undefined,
};
