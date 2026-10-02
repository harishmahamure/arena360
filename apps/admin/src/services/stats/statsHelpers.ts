import { round } from '@gaming-cafe/utils';
import type { RevenueByPaymentMethodDto } from './types';

export function asStatNumber(value: number | null | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

/** Backfill revenue fields missing from older cached API payloads. */
export function normalizeRevenue(
  revenue: Partial<RevenueByPaymentMethodDto> | null | undefined,
): RevenueByPaymentMethodDto {
  const planCashRevenue = asStatNumber(revenue?.planCashRevenue);
  const planOnlineRevenue = asStatNumber(revenue?.planOnlineRevenue);
  const planCreditRevenue = asStatNumber(revenue?.planCreditRevenue);
  const productCashRevenue = asStatNumber(revenue?.productCashRevenue);
  const productOnlineRevenue = asStatNumber(revenue?.productOnlineRevenue);
  const productCreditRevenue = asStatNumber(revenue?.productCreditRevenue);

  const planFromBreakdown = planCashRevenue + planOnlineRevenue + planCreditRevenue;
  const merchandiseFromBreakdown = productCashRevenue + productOnlineRevenue + productCreditRevenue;

  const plan = planFromBreakdown > 0 ? planFromBreakdown : asStatNumber(revenue?.plan);
  const merchandise =
    merchandiseFromBreakdown > 0 ? merchandiseFromBreakdown : asStatNumber(revenue?.merchandise);
  const total = plan + merchandise;

  return {
    plan,
    merchandise,
    total,
    cashRevenue: asStatNumber(revenue?.cashRevenue),
    onlineRevenue: asStatNumber(revenue?.onlineRevenue),
    creditRevenue: asStatNumber(revenue?.creditRevenue),
    planTransactionCount: asStatNumber(revenue?.planTransactionCount),
    productTransactionCount: asStatNumber(revenue?.productTransactionCount),
    planCashRevenue,
    planOnlineRevenue,
    planCreditRevenue,
    productCashRevenue,
    productOnlineRevenue,
    productCreditRevenue,
    planCashCount: asStatNumber(revenue?.planCashCount),
    planOnlineCount: asStatNumber(revenue?.planOnlineCount),
    planCreditCount: asStatNumber(revenue?.planCreditCount),
    productCashCount: asStatNumber(revenue?.productCashCount),
    productOnlineCount: asStatNumber(revenue?.productOnlineCount),
    productCreditCount: asStatNumber(revenue?.productCreditCount),
  };
}

export function calculatePeriodChange(current: number, previous?: number) {
  const curr = asStatNumber(current);
  const prev = asStatNumber(previous);

  if (prev === 0) {
    return { value: curr === 0 ? 0 : 100, positive: curr >= 0 };
  }

  const change = curr - prev;
  const pct = round((change / prev) * 100, 1);

  return {
    value: Number.isFinite(pct) ? pct : 0,
    positive: change >= 0,
  };
}

type TransactionType = 'plan' | 'product';

const TYPE_LABELS: Record<TransactionType, string> = {
  plan: 'Gaming plans',
  product: 'Merchandise',
};

/** Two-line subtitle: transaction count + payment amount breakdown. */
export function formatTypePaymentSubtitle(
  revenue: RevenueByPaymentMethodDto,
  type: TransactionType,
  formatCurrency: (amount: number) => string,
): string {
  const isPlan = type === 'plan';
  const transactionCount = isPlan ? revenue.planTransactionCount : revenue.productTransactionCount;
  const cashRevenue = isPlan ? revenue.planCashRevenue : revenue.productCashRevenue;
  const onlineRevenue = isPlan ? revenue.planOnlineRevenue : revenue.productOnlineRevenue;
  const creditRevenue = isPlan ? revenue.planCreditRevenue : revenue.productCreditRevenue;

  const line1 = `${transactionCount} transactions · ${TYPE_LABELS[type]}`;
  const line2 = `Cash: ${formatCurrency(cashRevenue)} | Online: ${formatCurrency(onlineRevenue)} | Credit: ${formatCurrency(creditRevenue)}`;

  return `${line1}\n${line2}`;
}

/** Payment-method transaction counts (plan + product combined). */
export function formatPaymentCountBreakdown(revenue: RevenueByPaymentMethodDto): string {
  const cashCount = revenue.planCashCount + revenue.productCashCount;
  const onlineCount = revenue.planOnlineCount + revenue.productOnlineCount;
  const creditCount = revenue.planCreditCount + revenue.productCreditCount;

  return `Cash: ${cashCount} | Online: ${onlineCount} | Credit: ${creditCount}`;
}
