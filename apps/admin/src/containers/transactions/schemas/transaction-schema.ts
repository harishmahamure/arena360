import {
  PaymentMethod,
  PaymentStatus,
  paymentMethodOptions,
  paymentStatusOptions,
  TransactionType,
  transactionTypeOptions,
} from '@gaming-cafe/contracts';
import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const PaymentMethodValues = PaymentMethod;
export const PaymentStatusValues = PaymentStatus;
export const TransactionTypeValues = TransactionType;

export type PaymentMethodType = (typeof PaymentMethod)[keyof typeof PaymentMethod];
export type PaymentStatusType = (typeof PaymentStatus)[keyof typeof PaymentStatus];
export type TransactionTypeType = (typeof TransactionType)[keyof typeof TransactionType];

export { paymentMethodOptions, paymentStatusOptions, transactionTypeOptions };

const productItemSchema = yup.object({
  productId: yup.string().uuid('Invalid product ID').required('Product is required'),
  quantity: yup
    .number()
    .integer('Quantity must be a whole number')
    .min(1, 'Quantity must be at least 1')
    .required('Quantity is required'),
  cashAmount: yup
    .number()
    .positive('Cash amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Split payment method is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),
});

export const createTransactionSchema = yup.object({
  playerId: yup.string().uuid('Invalid player ID').required(validationMessages.required('Player')),

  transactionType: yup
    .string()
    .oneOf(Object.values(TransactionType), 'Please select a valid transaction type')
    .required(validationMessages.required('Transaction Type'))
    .default(TransactionType.PRODUCT_PURCHASE),

  productIds: yup
    .array()
    .of(productItemSchema)
    .min(1, 'At least one product is required')
    .required('Products are required'),

  paymentMethod: yup
    .string()
    .oneOf(Object.values(PaymentMethod), 'Please select a valid payment method')
    .required(validationMessages.required('Payment Method')),

  notes: yup.string().max(500, 'Notes cannot exceed 500 characters').optional().nullable(),

  cashAmount: yup
    .number()
    .positive('Cash amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Cash amount is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),

  transactionDate: yup.string().optional().nullable(),
});

export type CreateTransactionFormData = yup.InferType<typeof createTransactionSchema>;

export const createTransactionDefaultValues: CreateTransactionFormData = {
  playerId: '',
  transactionType: TransactionType.PRODUCT_PURCHASE,
  productIds: [],
  paymentMethod: PaymentMethod.CASH,
  notes: '',
  transactionDate: undefined,
};

export const updateTransactionStatusSchema = yup.object({
  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatus), 'Please select a valid payment status')
    .required(validationMessages.required('Payment Status')),

  notes: yup.string().max(500, 'Notes cannot exceed 500 characters').optional().nullable(),
});

export type UpdateTransactionStatusFormData = yup.InferType<typeof updateTransactionStatusSchema>;

export const createPlanTransactionSchema = yup.object({
  playerId: yup
    .string()
    .uuid('Please select a valid player')
    .required(validationMessages.required('Player')),

  transactionType: yup
    .string()
    .oneOf(Object.values(TransactionType), 'Please select a valid transaction type')
    .required(validationMessages.required('Transaction Type')),

  planId: yup
    .string()
    .uuid('Please select a valid plan')
    .when('transactionType', {
      is: TransactionType.PLAN_PURCHASE,
      then: (schema) => schema.required('Plan is required for plan purchase'),
    }),
  cashAmount: yup
    .number()
    .positive('Cash amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Cash amount is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethod.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),

  paymentMethod: yup
    .string()
    .oneOf(Object.values(PaymentMethod), 'Please select a valid payment method')
    .required(validationMessages.required('Payment Method')),

  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatus), 'Please select a valid payment status')
    .optional()
    .default(PaymentStatus.PENDING),

  notes: yup.string().max(500, validationMessages.max('Notes', 500)).optional().nullable(),

  transactionDate: yup.date().optional().nullable(),
});

export const updatePlanTransactionStatusSchema = yup.object({
  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatus), 'Please select a valid payment status')
    .required(validationMessages.required('Payment Status')),

  notes: yup.string().max(500, validationMessages.max('Notes', 500)).optional().nullable(),
});

export type CreatePlanTransactionFormData = yup.InferType<typeof createPlanTransactionSchema>;

export type UpdatePlanTransactionStatusFormData = yup.InferType<
  typeof updatePlanTransactionStatusSchema
>;

export const createPlanTransactionDefaultValues: CreatePlanTransactionFormData = {
  playerId: '',
  transactionType: TransactionType.PLAN_PURCHASE,
  planId: '',
  paymentMethod: PaymentMethod.CASH,
  paymentStatus: PaymentStatus.PENDING,
  notes: '',
  transactionDate: undefined,
};

export const updatePlanTransactionStatusDefaultValues: UpdatePlanTransactionStatusFormData = {
  paymentStatus: PaymentStatus.COMPLETED,
  notes: '',
};
