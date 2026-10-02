import { getDashboardStats } from './getDashboardStats';
import type { StatsQueryDto, UserStatsDto } from './types';

/**
 * Fetches user and player statistics from dashboard stats
 */
export const getUserStats = async (filters?: StatsQueryDto): Promise<UserStatsDto> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.users;
};
