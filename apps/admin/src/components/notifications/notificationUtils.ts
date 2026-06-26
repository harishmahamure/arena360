import type { ActivityKindValue } from '@gaming-cafe/contracts';

export function getNotificationLink(
  entityType?: string | null,
  entityId?: string | null,
  payload?: Record<string, unknown> | null,
): string | undefined {
  if (!entityType || !entityId) return undefined;

  switch (entityType) {
    case 'transaction': {
      const transactionType =
        typeof payload?.transactionType === 'string' ? payload.transactionType : undefined;
      if (transactionType === 'plan_purchase') {
        return `/plan-transactions/${entityId}`;
      }
      return `/product-transactions/${entityId}`;
    }
    case 'credit_settlement':
      return `/credit/settlements/${entityId}`;
    case 'expense':
      return `/expenses/${entityId}`;
    case 'cash_deposit':
      return `/cash-deposits/${entityId}`;
    case 'stock_transfer_request':
      return `/inventory/transfers/${entityId}`;
    case 'stock_waste_event':
      return `/inventory/waste/${entityId}`;
    case 'session':
      return `/sessions/${entityId}`;
    case 'shift':
      return '/';
    case 'cash_register':
      return '/cash-registers';
    case 'device':
      return `/devices/${entityId}`;
    case 'kiosk_order':
      return `/kiosk-orders`;
    default:
      return undefined;
  }
}

export function formatRelativeTime(iso: string): string {
  const date = new Date(iso);
  const diffMs = Date.now() - date.getTime();
  const minutes = Math.floor(diffMs / 60000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return date.toLocaleDateString();
}

export function formatNotificationSummary(text: string): string {
  const words = text.split(/\s+/).map((word) => {
    if (word.includes('_')) {
      return word
        .split('_')
        .filter(Boolean)
        .map((part) => part.toLowerCase())
        .join(' ');
    }
    return word.toLowerCase();
  });

  if (words.length === 0) return text;
  words[0] = words[0].charAt(0).toUpperCase() + words[0].slice(1);
  return words.join(' ');
}

export function formatPaymentMethod(method: string): string {
  const labels: Record<string, string> = {
    cash: 'Cash',
    online: 'Online',
    split_payment: 'Split payment',
    credit: 'Credit',
  };
  return labels[method] ?? method.replace(/_/g, ' ');
}

export function formatSaleNotificationDetails(
  payload?: Record<string, unknown> | null,
): string | null {
  if (!payload) return null;

  const staffName = typeof payload.staffName === 'string' ? payload.staffName : null;
  const customerName = typeof payload.customerName === 'string' ? payload.customerName : null;
  const paymentLabel =
    typeof payload.paymentLabel === 'string'
      ? payload.paymentLabel
      : typeof payload.paymentMethod === 'string'
        ? formatPaymentMethod(payload.paymentMethod)
        : null;

  const parts: string[] = [];
  if (staffName) parts.push(`Staff: ${staffName}`);
  if (customerName) parts.push(`Customer: ${customerName}`);
  if (paymentLabel) parts.push(`Payment: ${paymentLabel}`);

  return parts.length > 0 ? parts.join(' · ') : null;
}

export function kindLabel(kind: ActivityKindValue | string): string {
  const labels: Record<string, string> = {
    transaction_sale: 'Sale',
    plan_sale: 'Plan sale',
    credit_settlement: 'Credit settlement',
    approval_requested: 'Approval',
    approval_decided: 'Decision',
    session_started: 'Session',
    session_ended: 'Session',
    device_status_changed: 'Device',
    shift_clock_in: 'Shift',
    shift_clock_out: 'Shift',
    shift_handover: 'Handover',
    cash_register_opened: 'Register',
    cash_register_closed: 'Register',
    cash_deposit_initiated: 'Deposit',
    inventory_transfer_requested: 'Transfer',
    inventory_waste_recorded: 'Waste',
    kiosk_order_placed: 'Kiosk order',
    kiosk_order_fulfilled: 'Order fulfilled',
    kiosk_order_cancelled: 'Order cancelled',
  };
  return labels[kind] ?? kind;
}
