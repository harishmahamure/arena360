import { useQuery } from '@tanstack/react-query';
import { getFinanceDepositStats, type StatsQueryDto } from '../services/stats';

export const useFinanceDepositStats = (filters?: StatsQueryDto) =>
  useQuery({
    queryKey: ['financeDepositStats', filters],
    queryFn: () => getFinanceDepositStats(filters),
  });
