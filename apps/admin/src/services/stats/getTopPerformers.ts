import { http } from '@gaming-cafe/utils';
import type { StatsQueryDto, TopPerformersDto } from './types';

/**
 * Fetches top performing plans, games, and players
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Top performers by revenue, session count, and spending
 */
export const getTopPerformers = async (filters?: StatsQueryDto) => {
  return http.get<TopPerformersDto>('/stats/top-performers', {
    params: {
      ...filters,
    },
  });
};
