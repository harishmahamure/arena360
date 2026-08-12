import { http } from '@gaming-cafe/utils';
import type { FinanceVarianceStatsDto, StatsQueryDto } from './types';

export const getFinanceVarianceStats = (filters?: StatsQueryDto) =>
  http.get<FinanceVarianceStatsDto>('/stats/finance/variance', {
    params: { ...filters },
  });
