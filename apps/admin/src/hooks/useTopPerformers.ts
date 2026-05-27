import { useQuery } from '@tanstack/react-query';
import { getTopPerformers, type StatsQueryDto } from '../services/stats';

/**
 * Hook to fetch top performers (plans, games, players)
 *
 * @param filters - Optional date filters for stats
 * @returns React Query result with top performers
 */
export const useTopPerformers = (filters?: StatsQueryDto) => {
  return useQuery({
    queryKey: ['topPerformers', filters],
    queryFn: () => getTopPerformers(filters),
  });
};
