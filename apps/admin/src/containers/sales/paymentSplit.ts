import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { PaymentStatus } from '@gaming-cafe/contracts';
import { formatCurrency } from '@gaming-cafe/utils';
import type { PlayerCreditDetail } from '../../services/credit';
import { PaymentMethodValues } from '../transactions/schemas/transaction-schema';

const SPLIT_TOLERANCE = 0.01;

export function isPosPaymentSuccessful(paymentStatus: PaymentStatusValue): boolean {
  return paymentStatus === PaymentStatus.COMPLETED || paymentStatus === PaymentStatus.CREDIT;
}

export function evaluateCreditBlocked(
  isCredit: boolean,
  purchaseAmount: number,
  creditDetail: PlayerCreditDetail | undefined,
  isCreditLoading: boolean,
): boolean {
  if (!isCredit) return false;
  if (isCreditLoading || !creditDetail?.summary) return true;

  const summary = creditDetail.summary;
  if (!summary.creditEnabled || summary.creditLimit <= 0) return true;
  return purchaseAmount > summary.available + SPLIT_TOLERANCE;
}

export function posSaleSuccessLabel(paymentMethod: string): string {
  return paymentMethod === PaymentMethodValues.CREDIT ? 'Credit sale recorded' : 'Sale complete';
}

export function validateSplitPaymentAmounts(
  total: number,
  cashAmount: string,
  onlineAmount: string,
): string | undefined {
  if (!cashAmount.trim() || !onlineAmount.trim()) {
    return 'Please enter both cash and online amounts for split payment';
  }

  const cash = Number.parseFloat(cashAmount);
  const online = Number.parseFloat(onlineAmount);

  if (Number.isNaN(cash) || Number.isNaN(online) || cash <= 0 || online <= 0) {
    return 'Cash and online amounts must be positive numbers';
  }

  if (Math.abs(cash + online - total) > SPLIT_TOLERANCE) {
    return `Cash and online must add up to ${formatCurrency(total, 'INR')}`;
  }

  return undefined;
}

export function formatPaymentSplit(row: {
  paymentMethod: string;
  amount: number;
  cashAmount?: number | null;
  onlineAmount?: number | null;
}): string {
  if (row.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT) {
    const cash = row.cashAmount ?? 0;
    const online = row.onlineAmount ?? 0;
    return `Cash: ${formatCurrency(cash, 'INR')} | Online: ${formatCurrency(online, 'INR')}`;
  }

  if (row.paymentMethod === PaymentMethodValues.CASH) {
    const cash = row.cashAmount ?? row.amount;
    return `Cash: ${formatCurrency(cash, 'INR')}`;
  }

  if (row.paymentMethod === PaymentMethodValues.ONLINE) {
    const online = row.onlineAmount ?? row.amount;
    return `Online: ${formatCurrency(online, 'INR')}`;
  }

  if (row.paymentMethod === PaymentMethodValues.CREDIT) {
    return `Credit: ${formatCurrency(row.amount, 'INR')}`;
  }

  return '—';
}
