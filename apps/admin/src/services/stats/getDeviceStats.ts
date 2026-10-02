import { getDashboardStats } from './getDashboardStats';
import type { DeviceStatsDto, StatsQueryDto } from './types';

/**
 * Fetches device statistics from dashboard stats
 */
export const getDeviceStats = async (filters?: StatsQueryDto): Promise<DeviceStatsDto> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.devices;
};
