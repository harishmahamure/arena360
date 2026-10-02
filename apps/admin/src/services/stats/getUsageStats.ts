import { http } from '@gaming-cafe/utils';
import type { PeriodPair, StatsQueryDto, UsageStatsDto } from './types';

/**
 * Fetches usage statistics including sessions and duration
 *
 * @param filters - Optional date filters (startDate, endDate as YYYY-MM-DD)
 * @returns Current and previous period usage stats
 */
export const getUsageStats = async (filters?: StatsQueryDto) => {
  return http.get<PeriodPair<UsageStatsDto>>('/stats/usage', {
    params: {
      ...filters,
    },
  });
};
