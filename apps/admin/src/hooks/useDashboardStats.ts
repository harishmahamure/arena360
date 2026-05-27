import { useQuery } from '@tanstack/react-query';
import { getDashboardStats, type StatsQueryDto } from '../services/stats';

export const useDashboardStats = (filters?: StatsQueryDto) => {
  return useQuery({
    queryKey: ['dashboardStats', filters],
    queryFn: () => getDashboardStats(filters),
  });
};
