import { http } from '@gaming-cafe/utils';
import type { FinanceReconciliationStatsDto, StatsQueryDto } from './types';

export const getFinanceReconciliationStats = (filters?: StatsQueryDto) =>
  http.get<FinanceReconciliationStatsDto>('/stats/finance/reconciliation', {
    params: { ...filters },
  });
