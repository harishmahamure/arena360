import { UnitType, unitTypeOptions } from '@gaming-cafe/contracts';
import { optionalString, stringWithLength } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const UnitTypeValues = UnitType;
export type UnitTypeType = (typeof UnitType)[keyof typeof UnitType];

export { unitTypeOptions };

export const createUnitSchema = yup.object({
  name: stringWithLength('Unit name', undefined, 100, true),

  abbreviation: stringWithLength('Unit abbreviation', undefined, 20, true),

  type: yup
    .string()
    .oneOf(Object.values(UnitType), 'Please select a valid unit type')
    .optional()
    .default(UnitType.OTHER),

  description: optionalString(),

  isActive: yup.boolean().optional().default(true),
});

export type CreateUnitFormData = yup.InferType<typeof createUnitSchema>;

export const createUnitDefaultValues: CreateUnitFormData = {
  name: '',
  abbreviation: '',
  type: UnitType.OTHER,
  description: '',
  isActive: true,
};
