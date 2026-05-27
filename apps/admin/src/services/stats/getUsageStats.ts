import { http } from '@gaming-cafe/utils';
import type { StatsQueryDto, UsageStatsDto } from './types';

/**
 * Fetches usage statistics including sessions and duration
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Session counts, total hours/minutes, and average session duration
 */
export const getUsageStats = async (filters?: StatsQueryDto) => {
  return http.get<UsageStatsDto>('/stats/usage', {
    params: {
      ...filters,
    },
  });
};
