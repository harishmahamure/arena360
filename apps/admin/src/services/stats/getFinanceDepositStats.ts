import { http } from '@gaming-cafe/utils';
import type { FinanceDepositStatsDto, StatsQueryDto } from './types';

export const getFinanceDepositStats = (filters?: StatsQueryDto) =>
  http.get<FinanceDepositStatsDto>('/stats/finance/deposits', {
    params: { ...filters },
  });
