import { getDashboardStats } from './getDashboardStats';
import type { PlanStatsDto, StatsQueryDto } from './types';

/**
 * Fetches plan statistics from dashboard stats
 */
export const getPlanStats = async (filters?: StatsQueryDto): Promise<PlanStatsDto> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.plans;
};
