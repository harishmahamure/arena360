import { optionalString, stringWithLength } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const UnitTypeValues = {
  BOX: 'box',
  KILOGRAM: 'kilogram',
  GRAM: 'gram',
  LITER: 'liter',
  MILLILITER: 'milliliter',
  PIECE: 'piece',
  PACK: 'pack',
  DOZEN: 'dozen',
  OTHER: 'other',
} as const;

export type UnitTypeType = (typeof UnitTypeValues)[keyof typeof UnitTypeValues];

export const unitTypeOptions = [
  { value: UnitTypeValues.BOX, label: 'Box' },
  { value: UnitTypeValues.KILOGRAM, label: 'Kilogram (kg)' },
  { value: UnitTypeValues.GRAM, label: 'Gram (g)' },
  { value: UnitTypeValues.LITER, label: 'Liter (L)' },
  { value: UnitTypeValues.MILLILITER, label: 'Milliliter (ml)' },
  { value: UnitTypeValues.PIECE, label: 'Piece' },
  { value: UnitTypeValues.PACK, label: 'Pack' },
  { value: UnitTypeValues.DOZEN, label: 'Dozen' },
  { value: UnitTypeValues.OTHER, label: 'Other' },
];

export const createUnitSchema = yup.object({
  name: stringWithLength('Unit name', undefined, 100, true),

  abbreviation: stringWithLength('Unit abbreviation', undefined, 20, true),

  type: yup
    .string()
    .oneOf(Object.values(UnitTypeValues), 'Please select a valid unit type')
    .optional()
    .default(UnitTypeValues.OTHER),

  description: optionalString(),

  isActive: yup.boolean().optional().default(true),
});

export type CreateUnitFormData = yup.InferType<typeof createUnitSchema>;

export const createUnitDefaultValues: CreateUnitFormData = {
  name: '',
  abbreviation: '',
  type: UnitTypeValues.OTHER,
  description: '',
  isActive: true,
};
