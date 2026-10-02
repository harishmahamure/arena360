import { getDashboardStats } from './getDashboardStats';
import type { RevenueTrendDto, StatsQueryDto } from './types';

/**
 * Fetches revenue trend data from dashboard stats
 */
export const getRevenueTrend = async (filters?: StatsQueryDto): Promise<RevenueTrendDto[]> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.revenueTrend;
};
