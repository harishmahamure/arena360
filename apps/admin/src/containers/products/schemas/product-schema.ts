import {
  nonNegativeNumberSchema,
  optionalString,
  stringWithLength,
  validationMessages,
} from '@gaming-cafe/utils';
import * as yup from 'yup';

export const ProductCategoryValues = {
  BEVERAGE: 'beverage',
  SNACK: 'snack',
  MEAL: 'meal',
  OTHER: 'other',
} as const;

export type ProductCategoryType =
  (typeof ProductCategoryValues)[keyof typeof ProductCategoryValues];

export const productCategoryOptions = [
  { value: ProductCategoryValues.BEVERAGE, label: 'Beverage' },
  { value: ProductCategoryValues.SNACK, label: 'Snack' },
  { value: ProductCategoryValues.MEAL, label: 'Meal' },
  { value: ProductCategoryValues.OTHER, label: 'Other' },
];

export const createProductSchema = yup.object({
  name: stringWithLength('Product name', undefined, 255, true),

  description: optionalString(),

  price: nonNegativeNumberSchema('Price'),

  category: yup
    .string()
    .oneOf(Object.values(ProductCategoryValues), 'Please select a valid category')
    .required(validationMessages.required('Category')),

  sku: yup.string().max(1000, validationMessages.max('SKU', 1000)).optional().nullable(),

  stockQuantity: yup
    .number()
    .integer('Stock quantity must be a whole number')
    .min(0, 'Stock quantity cannot be negative')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value)),

  isActive: yup.boolean().optional().default(true),
});

export type CreateProductFormData = yup.InferType<typeof createProductSchema>;

export const createProductDefaultValues: CreateProductFormData = {
  name: '',
  description: '',
  price: 0,
  category: ProductCategoryValues.OTHER,
  sku: '',
  stockQuantity: 0,
  isActive: true,
};

export const productSchema = yup.object({
  name: stringWithLength('Name', undefined, 255, true),
  price: nonNegativeNumberSchema('Price'),
  quantity: nonNegativeNumberSchema('Quantity'),
  status: yup.string().oneOf(['active', 'inactive'], 'Status must be active or inactive'),
});
