/**
 * Postgres enum labels — keep in sync with the live database.
 * Source: public.*_enum types (see apps/backend/scripts/list-db-enums.mjs).
 */

export type SelectOption<T extends string = string> = { value: T; label: string };

// --- Devices (devices_* / plans_* device scope) ---

export const DeviceType = {
  PC: 'PC',
  CONSOLE: 'CONSOLE',
  PS5: 'PS5',
  PS4: 'PS4',
  OTHER: 'OTHER',
} as const;
export type DeviceTypeValue = (typeof DeviceType)[keyof typeof DeviceType];
export const DEVICE_TYPE_VALUES = Object.values(DeviceType);

export const DeviceSubType = {
  HIGH_END_PCS: 'HIGH_END_PCS',
  MID_RANGE_PCS: 'MID_RANGE_PCS',
  PREMIUM_TV_CONSOLES: 'PREMIUM_TV_CONSOLES',
  STANDARD_TV_CONSOLES: 'STANDARD_TV_CONSOLES',
  OTHER: 'OTHER',
} as const;
export type DeviceSubTypeValue = (typeof DeviceSubType)[keyof typeof DeviceSubType];
export const DEVICE_SUB_TYPE_VALUES = Object.values(DeviceSubType);

export const DeviceStatus = {
  OPERATIONAL: 'operational',
  UNDER_MAINTENANCE: 'under_maintenance',
  OUT_OF_SERVICE: 'out_of_service',
  IN_USE: 'in_use',
  AVAILABLE: 'available',
} as const;
export type DeviceStatusValue = (typeof DeviceStatus)[keyof typeof DeviceStatus];
export const DEVICE_STATUS_VALUES = Object.values(DeviceStatus);

export const DeviceRegistrationStatus = {
  REGISTERED: 'registered',
  UNREGISTERED: 'unregistered',
} as const;

// --- Plans ---

export const PlanType = {
  TIME_BASED: 'time_based',
  SESSION_BASED: 'session_based',
  UNLIMITED_DAILY: 'unlimited_daily',
  HOURLY_RENTAL: 'hourly_rental',
  MONTHLY_SUBSCRIPTION: 'monthly_subscription',
  WEEKEND_SPECIAL: 'weekend_special',
} as const;
export type PlanTypeValue = (typeof PlanType)[keyof typeof PlanType];
/** Supported in admin UI + plan_service today */
export const PLAN_TYPE_ADMIN_VALUES = [PlanType.TIME_BASED, PlanType.WEEKEND_SPECIAL] as const;

export const PlanKind = {
  TIME: 'time',
  HAPPY_HOURS: 'happy_hours',
} as const;

export const BalanceStatus = {
  ACTIVE: 'active',
  EXPIRED: 'expired',
  EXHAUSTED: 'exhausted',
  CANCELLED: 'cancelled',
} as const;
export type BalanceStatusValue = (typeof BalanceStatus)[keyof typeof BalanceStatus];

export const PlayerPlanStatus = {
  ACTIVE: 'active',
  EXPIRED: 'expired',
  EXHAUSTED: 'exhausted',
  CANCELLED: 'cancelled',
  MOVED_TO_NEXT_PLAN: 'moved_to_next_plan',
} as const;
export type PlayerPlanStatusValue = (typeof PlayerPlanStatus)[keyof typeof PlayerPlanStatus];

// --- Products ---

export const ProductCategory = {
  BEVERAGE: 'beverage',
  SNACK: 'snack',
  MEAL: 'meal',
  OTHER: 'other',
} as const;
export type ProductCategoryValue = (typeof ProductCategory)[keyof typeof ProductCategory];
export const PRODUCT_CATEGORY_VALUES = Object.values(ProductCategory);

// --- Units ---

export const UnitType = {
  PIECE: 'piece',
  BOX: 'box',
  CARTON: 'carton',
  PACK: 'pack',
  BOTTLE: 'bottle',
  CAN: 'can',
  KILOGRAM: 'kilogram',
  GRAM: 'gram',
  LITER: 'liter',
  MILLILITER: 'milliliter',
  OTHER: 'other',
} as const;
export type UnitTypeValue = (typeof UnitType)[keyof typeof UnitType];
export const UNIT_TYPE_VALUES = Object.values(UnitType);

// --- Transactions ---

export const TransactionType = {
  PLAN_PURCHASE: 'plan_purchase',
  PRODUCT_PURCHASE: 'product_purchase',
} as const;
export type TransactionTypeValue = (typeof TransactionType)[keyof typeof TransactionType];

export const PaymentMethod = {
  CASH: 'cash',
  ONLINE: 'online',
  SPLIT_PAYMENT: 'split_payment',
  CREDIT: 'credit',
} as const;
export type PaymentMethodValue = (typeof PaymentMethod)[keyof typeof PaymentMethod];

export const PaymentStatus = {
  PENDING: 'pending',
  COMPLETED: 'completed',
  FAILED: 'failed',
  REFUNDED: 'refunded',
  CREDIT: 'credit',
} as const;
export type PaymentStatusValue = (typeof PaymentStatus)[keyof typeof PaymentStatus];

// --- Expenses ---

export const ExpenseType = {
  SALARIES: 'salaries',
  RENT: 'rent',
  UTILITIES: 'utilities',
  MAINTENANCE: 'maintenance',
  MARKETING: 'marketing',
  SUPPLIES: 'supplies',
  INTERNET: 'internet',
  ELECTRICITY: 'electricity',
  WATER: 'water',
  INSURANCE: 'insurance',
  TAX: 'tax',
  MISC: 'misc',
} as const;

export const ExpensePaymentMethod = {
  CASH: 'cash',
  ONLINE: 'online',
  BANK_TRANSFER: 'bank_transfer',
  CHEQUE: 'cheque',
} as const;

export const ExpenseStatus = {
  PENDING: 'pending',
  APPROVED: 'approved',
  PAID: 'paid',
  REJECTED: 'rejected',
} as const;

// --- UI option helpers ---

export const deviceTypeOptions: SelectOption<DeviceTypeValue>[] = [
  { value: DeviceType.PC, label: 'PC' },
  { value: DeviceType.CONSOLE, label: 'Console' },
  { value: DeviceType.PS5, label: 'PS5' },
  { value: DeviceType.PS4, label: 'PS4' },
  { value: DeviceType.OTHER, label: 'Other' },
];

export const deviceSubTypeOptions: SelectOption<DeviceSubTypeValue>[] = [
  { value: DeviceSubType.HIGH_END_PCS, label: 'High End PCs' },
  { value: DeviceSubType.MID_RANGE_PCS, label: 'Mid Range PCs' },
  { value: DeviceSubType.PREMIUM_TV_CONSOLES, label: 'Premium TV Consoles' },
  { value: DeviceSubType.STANDARD_TV_CONSOLES, label: 'Standard TV Consoles' },
  { value: DeviceSubType.OTHER, label: 'Other' },
];

export const deviceStatusOptions: SelectOption<DeviceStatusValue>[] = [
  { value: DeviceStatus.OPERATIONAL, label: 'Operational' },
  { value: DeviceStatus.UNDER_MAINTENANCE, label: 'Under Maintenance' },
  { value: DeviceStatus.OUT_OF_SERVICE, label: 'Out of Service' },
  { value: DeviceStatus.IN_USE, label: 'In Use' },
  { value: DeviceStatus.AVAILABLE, label: 'Available' },
];

export const planTypeOptions: SelectOption<(typeof PLAN_TYPE_ADMIN_VALUES)[number]>[] = [
  { value: PlanType.TIME_BASED, label: 'Time Plan' },
  { value: PlanType.WEEKEND_SPECIAL, label: 'Happy Hours' },
];

export const productCategoryOptions: SelectOption<ProductCategoryValue>[] = [
  { value: ProductCategory.BEVERAGE, label: 'Beverage' },
  { value: ProductCategory.SNACK, label: 'Snack' },
  { value: ProductCategory.MEAL, label: 'Meal' },
  { value: ProductCategory.OTHER, label: 'Other' },
];

export const unitTypeOptions: SelectOption<UnitTypeValue>[] = [
  { value: UnitType.PIECE, label: 'Piece' },
  { value: UnitType.BOX, label: 'Box' },
  { value: UnitType.CARTON, label: 'Carton' },
  { value: UnitType.PACK, label: 'Pack' },
  { value: UnitType.BOTTLE, label: 'Bottle' },
  { value: UnitType.CAN, label: 'Can' },
  { value: UnitType.KILOGRAM, label: 'Kilogram (kg)' },
  { value: UnitType.GRAM, label: 'Gram (g)' },
  { value: UnitType.LITER, label: 'Liter (L)' },
  { value: UnitType.MILLILITER, label: 'Milliliter (ml)' },
  { value: UnitType.OTHER, label: 'Other' },
];

export const paymentMethodOptions: SelectOption<PaymentMethodValue>[] = [
  { value: PaymentMethod.CASH, label: 'Cash' },
  { value: PaymentMethod.ONLINE, label: 'Online' },
  { value: PaymentMethod.SPLIT_PAYMENT, label: 'Split Payment' },
  { value: PaymentMethod.CREDIT, label: 'Credit (Tab)' },
];

export const paymentStatusOptions: SelectOption<PaymentStatusValue>[] = [
  { value: PaymentStatus.PENDING, label: 'Pending' },
  { value: PaymentStatus.COMPLETED, label: 'Completed' },
  { value: PaymentStatus.FAILED, label: 'Failed' },
  { value: PaymentStatus.REFUNDED, label: 'Refunded' },
  { value: PaymentStatus.CREDIT, label: 'Credit' },
];

export const transactionTypeOptions: SelectOption<TransactionTypeValue>[] = [
  { value: TransactionType.PLAN_PURCHASE, label: 'Plan Purchase' },
  { value: TransactionType.PRODUCT_PURCHASE, label: 'Product Purchase' },
];
