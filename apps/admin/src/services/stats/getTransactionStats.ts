import { http } from '@gaming-cafe/utils';
import type { StatsQueryDto, TransactionStatsDto } from './types';

/**
 * Fetches transaction statistics
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Transaction counts and average transaction amount
 */
export const getTransactionStats = async (filters?: StatsQueryDto) => {
  return http.get<TransactionStatsDto>('/stats/transactions', {
    params: {
      ...filters,
    },
  });
};
