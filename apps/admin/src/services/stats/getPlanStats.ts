import { http } from '@gaming-cafe/utils';
import type { PlanStatsDto, StatsQueryDto } from './types';

/**
 * Fetches plan statistics including active/expired plans and breakdown by type
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Plan counts and revenue breakdown by plan type
 */
export const getPlanStats = async (filters?: StatsQueryDto) => {
  return http.get<PlanStatsDto>('/stats/plans', {
    params: {
      ...filters,
    },
  });
};
