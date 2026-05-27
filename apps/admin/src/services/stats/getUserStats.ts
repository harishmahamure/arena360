import { http } from '@gaming-cafe/utils';
import type { StatsQueryDto, UserStatsDto } from './types';

/**
 * Fetches user and player statistics
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns User and player counts including active users and new users in period
 */
export const getUserStats = async (filters?: StatsQueryDto) => {
  return http.get<UserStatsDto>('/stats/users', {
    params: {
      ...filters,
    },
  });
};
