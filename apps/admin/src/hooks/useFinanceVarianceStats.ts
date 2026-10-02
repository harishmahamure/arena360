import { useQuery } from '@tanstack/react-query';
import { getFinanceVarianceStats, type StatsQueryDto } from '../services/stats';

export const useFinanceVarianceStats = (filters?: StatsQueryDto) =>
  useQuery({
    queryKey: ['financeVarianceStats', filters],
    queryFn: () => getFinanceVarianceStats(filters),
  });
