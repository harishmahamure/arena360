/**
 * Activity kinds for persisted notifications — keep aligned with Postgres activity_kind enum.
 */

export const ActivityKind = {
  TRANSACTION_SALE: 'transaction_sale',
  PLAN_SALE: 'plan_sale',
  CREDIT_SETTLEMENT: 'credit_settlement',
  APPROVAL_REQUESTED: 'approval_requested',
  APPROVAL_DECIDED: 'approval_decided',
  SESSION_STARTED: 'session_started',
  SESSION_ENDED: 'session_ended',
  DEVICE_STATUS_CHANGED: 'device_status_changed',
  SHIFT_CLOCK_IN: 'shift_clock_in',
  SHIFT_CLOCK_OUT: 'shift_clock_out',
  SHIFT_HANDOVER: 'shift_handover',
  CASH_REGISTER_OPENED: 'cash_register_opened',
  CASH_REGISTER_CLOSED: 'cash_register_closed',
  CASH_DEPOSIT_INITIATED: 'cash_deposit_initiated',
  INVENTORY_TRANSFER_REQUESTED: 'inventory_transfer_requested',
  INVENTORY_WASTE_RECORDED: 'inventory_waste_recorded',
  KIOSK_ORDER_PLACED: 'kiosk_order_placed',
  KIOSK_ORDER_FULFILLED: 'kiosk_order_fulfilled',
  KIOSK_ORDER_CANCELLED: 'kiosk_order_cancelled',
} as const;

export type ActivityKindValue = (typeof ActivityKind)[keyof typeof ActivityKind];

export const ACTIVITY_KIND_VALUES = Object.values(ActivityKind);

/** Actionable staff alerts — bell badge, live toasts, and default inbox filter. */
export const STAFF_IMPORTANT_NOTIFICATION_KINDS = [
  ActivityKind.APPROVAL_REQUESTED,
  ActivityKind.APPROVAL_DECIDED,
  ActivityKind.KIOSK_ORDER_PLACED,
  ActivityKind.KIOSK_ORDER_CANCELLED,
  ActivityKind.CASH_DEPOSIT_INITIATED,
  ActivityKind.INVENTORY_TRANSFER_REQUESTED,
] as const;

export type StaffImportantNotificationKind = (typeof STAFF_IMPORTANT_NOTIFICATION_KINDS)[number];

const STAFF_IMPORTANT_KIND_SET = new Set<string>(STAFF_IMPORTANT_NOTIFICATION_KINDS);

export function isImportantNotificationKind(kind: string): kind is StaffImportantNotificationKind {
  return STAFF_IMPORTANT_KIND_SET.has(kind);
}

export const activityKindLabels: Record<ActivityKindValue, string> = {
  [ActivityKind.TRANSACTION_SALE]: 'Product sale',
  [ActivityKind.PLAN_SALE]: 'Plan sale',
  [ActivityKind.CREDIT_SETTLEMENT]: 'Credit settlement',
  [ActivityKind.APPROVAL_REQUESTED]: 'Approval requested',
  [ActivityKind.APPROVAL_DECIDED]: 'Approval decided',
  [ActivityKind.SESSION_STARTED]: 'Session started',
  [ActivityKind.SESSION_ENDED]: 'Session ended',
  [ActivityKind.DEVICE_STATUS_CHANGED]: 'Device status',
  [ActivityKind.SHIFT_CLOCK_IN]: 'Shift clock-in',
  [ActivityKind.SHIFT_CLOCK_OUT]: 'Shift clock-out',
  [ActivityKind.SHIFT_HANDOVER]: 'Shift handover',
  [ActivityKind.CASH_REGISTER_OPENED]: 'Register opened',
  [ActivityKind.CASH_REGISTER_CLOSED]: 'Register closed',
  [ActivityKind.CASH_DEPOSIT_INITIATED]: 'Cash deposit',
  [ActivityKind.INVENTORY_TRANSFER_REQUESTED]: 'Stock transfer',
  [ActivityKind.INVENTORY_WASTE_RECORDED]: 'Stock waste',
  [ActivityKind.KIOSK_ORDER_PLACED]: 'Kiosk order',
  [ActivityKind.KIOSK_ORDER_FULFILLED]: 'Kiosk order fulfilled',
  [ActivityKind.KIOSK_ORDER_CANCELLED]: 'Kiosk order cancelled',
};
