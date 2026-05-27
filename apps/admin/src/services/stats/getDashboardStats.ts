import { http } from '@gaming-cafe/utils';
import type { DashboardStatsDto, StatsQueryDto } from './types';

export const getDashboardStats = async (filters?: StatsQueryDto) => {
  return http.get<DashboardStatsDto>('/stats/dashboard', {
    params: {
      ...filters,
    },
  });
};
