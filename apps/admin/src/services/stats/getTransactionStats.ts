import { getDashboardStats } from './getDashboardStats';
import type { StatsQueryDto, TransactionStatsDto } from './types';

/**
 * Fetches transaction statistics for the current period from dashboard stats
 */
export const getTransactionStats = async (
  filters?: StatsQueryDto,
): Promise<TransactionStatsDto> => {
  const dashboard = await getDashboardStats(filters);
  return dashboard.transactions.current;
};
