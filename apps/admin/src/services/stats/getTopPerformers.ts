import { getDashboardStats } from './getDashboardStats';
import type { StatsQueryDto, TopPerformersDto } from './types';

/**
 * Fetches top performing plans, games, and players from dashboard stats
 */
export const getTopPerformers = async (filters?: StatsQueryDto): Promise<TopPerformersDto> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.topPerformers;
};
