import { http } from '@gaming-cafe/utils';
import type { RevenueTrendDto, StatsQueryDto } from './types';

/**
 * Fetches revenue trend data over time
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Daily revenue trends by payment method
 */
export const getRevenueTrend = async (filters?: StatsQueryDto) => {
  return http.get<RevenueTrendDto[]>('/stats/revenue-trend', {
    params: {
      ...filters,
    },
  });
};
