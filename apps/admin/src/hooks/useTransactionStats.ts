import { useQuery } from '@tanstack/react-query';
import { getTransactionStats, type StatsQueryDto } from '../services/stats';

/**
 * Hook to fetch transaction statistics
 *
 * @param filters - Optional date filters for stats
 * @returns React Query result with transaction stats
 */
export const useTransactionStats = (filters?: StatsQueryDto) => {
  return useQuery({
    queryKey: ['transactionStats', filters],
    queryFn: () => getTransactionStats(filters),
  });
};
