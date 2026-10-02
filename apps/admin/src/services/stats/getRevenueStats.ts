import { http } from '@gaming-cafe/utils';
import type { PeriodPair, RevenueByPaymentMethodDto, StatsQueryDto } from './types';

/**
 * Fetches revenue statistics broken down by payment method
 *
 * @param filters - Optional date filters (startDate, endDate as YYYY-MM-DD)
 * @returns Current and previous period revenue breakdown
 */
export const getRevenueStats = async (filters?: StatsQueryDto) => {
  return http.get<PeriodPair<RevenueByPaymentMethodDto>>('/stats/revenue/by-payment-method', {
    params: {
      ...filters,
    },
  });
};
