import { useQuery } from '@tanstack/react-query';
import { getFinanceReconciliationStats, type StatsQueryDto } from '../services/stats';

export const useFinanceReconciliationStats = (filters?: StatsQueryDto) =>
  useQuery({
    queryKey: ['financeReconciliationStats', filters],
    queryFn: () => getFinanceReconciliationStats(filters),
  });
