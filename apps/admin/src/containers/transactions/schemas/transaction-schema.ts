import { validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

// Enum values
export const PaymentMethodValues = {
  CASH: 'cash',
  ONLINE: 'online',
  SPLIT_PAYMENT: 'split_payment',
  CREDIT: 'credit',
} as const;

export const PaymentStatusValues = {
  PENDING: 'pending',
  COMPLETED: 'completed',
  FAILED: 'failed',
  REFUNDED: 'refunded',
  CANCELLED: 'cancelled',
  CREDIT: 'credit',
} as const;

export const TransactionTypeValues = {
  PLAN_PURCHASE: 'plan_purchase',
  PRODUCT_PURCHASE: 'product_purchase',
} as const;

export type PaymentMethodType = (typeof PaymentMethodValues)[keyof typeof PaymentMethodValues];

export type PaymentStatusType = (typeof PaymentStatusValues)[keyof typeof PaymentStatusValues];

export type TransactionTypeType =
  (typeof TransactionTypeValues)[keyof typeof TransactionTypeValues];

// Options for select fields
export const paymentMethodOptions = [
  { value: PaymentMethodValues.CASH, label: 'Cash' },
  { value: PaymentMethodValues.ONLINE, label: 'Online' },
  { value: PaymentMethodValues.SPLIT_PAYMENT, label: 'Split Payment' },
  { value: PaymentMethodValues.CREDIT, label: 'Credit (Tab)' },
];

export const paymentStatusOptions = [
  { value: PaymentStatusValues.PENDING, label: 'Pending' },
  { value: PaymentStatusValues.COMPLETED, label: 'Completed' },
  { value: PaymentStatusValues.FAILED, label: 'Failed' },
  { value: PaymentStatusValues.REFUNDED, label: 'Refunded' },
  { value: PaymentStatusValues.CANCELLED, label: 'Cancelled' },
];

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
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Split payment method is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),
});

// Create transaction schema
export const createTransactionSchema = yup.object({
  playerId: yup.string().uuid('Invalid player ID').required(validationMessages.required('Player')),

  transactionType: yup
    .string()
    .oneOf(Object.values(TransactionTypeValues), 'Please select a valid transaction type')
    .required(validationMessages.required('Transaction Type'))
    .default(TransactionTypeValues.PRODUCT_PURCHASE),

  productIds: yup
    .array()
    .of(productItemSchema)
    .min(1, 'At least one product is required')
    .required('Products are required'),

  paymentMethod: yup
    .string()
    .oneOf(Object.values(PaymentMethodValues), 'Please select a valid payment method')
    .required(validationMessages.required('Payment Method')),

  notes: yup.string().max(500, 'Notes cannot exceed 500 characters').optional().nullable(),

  cashAmount: yup
    .number()
    .positive('Cash amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Cash amount is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),

  transactionDate: yup.string().optional().nullable(),
});

export type CreateTransactionFormData = yup.InferType<typeof createTransactionSchema>;

export const createTransactionDefaultValues: CreateTransactionFormData = {
  playerId: '',
  transactionType: TransactionTypeValues.PRODUCT_PURCHASE,
  productIds: [],
  paymentMethod: PaymentMethodValues.CASH,
  notes: '',
  transactionDate: undefined,
};

// Update transaction status schema
export const updateTransactionStatusSchema = yup.object({
  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatusValues), 'Please select a valid payment status')
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
    .oneOf(Object.values(TransactionTypeValues), 'Please select a valid transaction type')
    .required(validationMessages.required('Transaction Type')),

  planId: yup
    .string()
    .uuid('Please select a valid plan')
    .when('transactionType', {
      is: TransactionTypeValues.PLAN_PURCHASE,
      then: (schema) => schema.required('Plan is required for plan purchase'),
    }),
  cashAmount: yup
    .number()
    .positive('Cash amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Cash amount is required'),
    }),
  onlineAmount: yup
    .number()
    .positive('Online amount must be positive')
    .when('paymentMethod', {
      is: PaymentMethodValues.SPLIT_PAYMENT,
      then: (schema) => schema.required('Online amount is required'),
    }),

  paymentMethod: yup
    .string()
    .oneOf(Object.values(PaymentMethodValues), 'Please select a valid payment method')
    .required(validationMessages.required('Payment Method')),

  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatusValues), 'Please select a valid payment status')
    .optional()
    .default(PaymentStatusValues.PENDING),

  notes: yup.string().max(500, validationMessages.max('Notes', 500)).optional().nullable(),

  transactionDate: yup.date().optional().nullable(),
});

export const updatePlanTransactionStatusSchema = yup.object({
  paymentStatus: yup
    .string()
    .oneOf(Object.values(PaymentStatusValues), 'Please select a valid payment status')
    .required(validationMessages.required('Payment Status')),

  notes: yup.string().max(500, validationMessages.max('Notes', 500)).optional().nullable(),
});

export type CreatePlanTransactionFormData = yup.InferType<typeof createPlanTransactionSchema>;

export type UpdatePlanTransactionStatusFormData = yup.InferType<
  typeof updatePlanTransactionStatusSchema
>;

export const createPlanTransactionDefaultValues: CreatePlanTransactionFormData = {
  playerId: '',
  transactionType: TransactionTypeValues.PLAN_PURCHASE,
  planId: '',
  paymentMethod: PaymentMethodValues.CASH,
  paymentStatus: PaymentStatusValues.PENDING,
  notes: '',
  transactionDate: undefined,
};

export const updatePlanTransactionStatusDefaultValues: UpdatePlanTransactionStatusFormData = {
  paymentStatus: PaymentStatusValues.COMPLETED,
  notes: '',
};
