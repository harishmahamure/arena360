import { describe, expect, it } from 'vitest';
import {
  formatNotificationSummary,
  formatRelativeTime,
  formatSaleNotificationDetails,
  getNotificationLink,
  kindLabel,
} from './notificationUtils';

describe('notificationUtils', () => {
  it('builds entity links', () => {
    expect(getNotificationLink('credit_settlement', 'abc-123')).toBe('/credit/settlements/abc-123');
    expect(getNotificationLink('transaction', 'tx-1')).toBe('/product-transactions/tx-1');
    expect(getNotificationLink('kiosk_order', 'ord-1')).toBe('/kiosk-orders');
    expect(getNotificationLink('transaction', 'tx-2', { transactionType: 'plan_purchase' })).toBe(
      '/plan-transactions/tx-2',
    );
    expect(getNotificationLink('unknown', 'x')).toBeUndefined();
  });

  it('formats relative time', () => {
    const recent = new Date(Date.now() - 2 * 60_000).toISOString();
    expect(formatRelativeTime(recent)).toBe('2m ago');
  });

  it('maps kind labels', () => {
    expect(kindLabel('credit_settlement')).toBe('Credit settlement');
    expect(kindLabel('approval_decided')).toBe('Decision');
  });

  it('formats notification summaries', () => {
    expect(formatNotificationSummary('plan_purchase completed')).toBe('Plan purchase completed');
    expect(formatNotificationSummary('Awaiting admin approval')).toBe('Awaiting admin approval');
  });

  it('formats sale notification details from payload', () => {
    expect(
      formatSaleNotificationDetails({
        staffName: 'Alex',
        customerName: 'Jordan',
        paymentLabel: 'Cash',
      }),
    ).toBe('Staff: Alex · Customer: Jordan · Payment: Cash');
  });
});
