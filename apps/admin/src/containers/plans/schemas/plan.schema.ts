import {
  DEVICE_SUB_TYPE_VALUES,
  DEVICE_TYPE_VALUES,
  type DeductionProfile,
  PLAN_TYPE_ADMIN_VALUES,
  PlanType,
  validateDeductionProfile,
} from '@gaming-cafe/contracts';
import {
  currencySchema,
  normalizeTimeOfDay,
  optionalString,
  stringWithLength,
  validationMessages,
} from '@gaming-cafe/utils';
import * as yup from 'yup';

export {
  DeviceSubType,
  DeviceType,
  deviceSubTypeOptions,
  deviceTypeOptions,
  PlanType,
} from '@gaming-cafe/contracts';

export const PlanTypeValues = PlanType;
export type PlanTypeType = (typeof PlanType)[keyof typeof PlanType];

export const createPlanSchema = yup
  .object({
    name: stringWithLength('Plan name', undefined, 100, true),

    description: optionalString(),

    price: currencySchema('Price'),

    planType: yup
      .string()
      .oneOf([...PLAN_TYPE_ADMIN_VALUES], 'Please select a valid plan type')
      .required(validationMessages.required('Plan Type'))
      .default(PlanType.TIME_BASED),

    validityDays: yup
      .number()
      .positive('Validity days must be greater than 0')
      .integer('Validity days must be a whole number')
      .required(validationMessages.required('Validity days'))
      .default(7),

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

    dynamicDeductionEnabled: yup.boolean().optional().default(false),

    peakWindowStart: yup
      .string()
      .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$/, {
        message: 'Time must be in HH:MM or HH:MM:SS format',
        excludeEmptyString: true,
      })
      .when('dynamicDeductionEnabled', {
        is: true,
        then: (schema) => schema.required('Peak window start is required'),
      }),

    peakWindowEnd: yup
      .string()
      .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$/, {
        message: 'Time must be in HH:MM or HH:MM:SS format',
        excludeEmptyString: true,
      })
      .when('dynamicDeductionEnabled', {
        is: true,
        then: (schema) => schema.required('Peak window end is required'),
      }),

    peakRatio: yup.number().when('dynamicDeductionEnabled', {
      is: true,
      then: (schema) =>
        schema.required('Peak ratio is required').moreThan(1, 'Peak ratio must be greater than 1'),
    }),

    lowWindowStart: yup
      .string()
      .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$/, {
        message: 'Time must be in HH:MM or HH:MM:SS format',
        excludeEmptyString: true,
      })
      .when('dynamicDeductionEnabled', {
        is: true,
        then: (schema) => schema.required('Low window start is required'),
      }),

    lowWindowEnd: yup
      .string()
      .matches(/^([0-1]?[0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9])?$/, {
        message: 'Time must be in HH:MM or HH:MM:SS format',
        excludeEmptyString: true,
      })
      .when('dynamicDeductionEnabled', {
        is: true,
        then: (schema) => schema.required('Low window end is required'),
      }),

    lowRatio: yup.number().when('dynamicDeductionEnabled', {
      is: true,
      then: (schema) =>
        schema
          .required('Low ratio is required')
          .moreThan(0, 'Low ratio must be greater than 0')
          .lessThan(1, 'Low ratio must be less than 1'),
    }),

    deductionPreviewNote: yup.string().optional(),
  })
  .test('deduction-profile', 'Invalid deduction profile', function (value) {
    if (!value.dynamicDeductionEnabled) return true;
    const profile: DeductionProfile = {
      peakWindowStart: normalizeTimeOfDay(value.peakWindowStart),
      peakWindowEnd: normalizeTimeOfDay(value.peakWindowEnd),
      peakRatio: value.peakRatio ?? 0,
      lowWindowStart: normalizeTimeOfDay(value.lowWindowStart),
      lowWindowEnd: normalizeTimeOfDay(value.lowWindowEnd),
      lowRatio: value.lowRatio ?? 0,
    };
    const error = validateDeductionProfile(profile);
    if (error) return this.createError({ message: error });
    return true;
  });

export type CreatePlanFormData = yup.InferType<typeof createPlanSchema>;

export const createPlanDefaultValues: CreatePlanFormData = {
  name: '',
  description: '',
  price: 0,
  planType: PlanType.TIME_BASED,
  validityDays: 7,
  timeCredits: 300,
  isActive: true,
  deviceType: undefined,
  deviceSubType: undefined,
  dynamicDeductionEnabled: false,
  peakWindowStart: undefined,
  peakWindowEnd: undefined,
  peakRatio: 1.5,
  lowWindowStart: undefined,
  lowWindowEnd: undefined,
  lowRatio: 0.8,
  deductionPreviewNote: undefined,
};
