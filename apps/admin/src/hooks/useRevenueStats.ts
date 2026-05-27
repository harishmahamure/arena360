import { useQuery } from '@tanstack/react-query';
import { getRevenueStats, type StatsQueryDto } from '../services/stats';

/**
 * Hook to fetch revenue statistics
 *
 * @param filters - Optional date filters for stats
 * @returns React Query result with revenue stats
 */
export const useRevenueStats = (filters?: StatsQueryDto) => {
  return useQuery({
    queryKey: ['revenueStats', filters],
    queryFn: () => getRevenueStats(filters),
  });
};
