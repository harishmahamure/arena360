import { http } from '@gaming-cafe/utils';
import { normalizeRevenue } from './statsHelpers';
import type { DashboardStatsDto, StatsQueryDto } from './types';

export const getDashboardStats = async (filters?: StatsQueryDto) => {
  const data = await http.get<DashboardStatsDto>('/stats/dashboard', {
    params: {
      ...filters,
    },
  });

  return {
    ...data,
    revenue: {
      current: normalizeRevenue(data.revenue.current),
      previous: normalizeRevenue(data.revenue.previous),
    },
  };
};
