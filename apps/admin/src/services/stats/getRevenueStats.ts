import { http } from '@gaming-cafe/utils';
import type { RevenueByPaymentMethodDto, StatsQueryDto } from './types';

/**
 * Fetches revenue statistics broken down by payment method
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Revenue breakdown by cash, online, and total
 */
export const getRevenueStats = async (filters?: StatsQueryDto) => {
  return http.get<RevenueByPaymentMethodDto>('/stats/revenue', {
    params: {
      ...filters,
    },
  });
};
