import { http } from '@gaming-cafe/utils';
import type { DeviceStatsDto, StatsQueryDto } from './types';

/**
 * Fetches device statistics including utilization metrics
 *
 * @param filters - Optional date filters (startDate, endDate)
 * @returns Device counts and detailed utilization per device
 */
export const getDeviceStats = async (filters?: StatsQueryDto) => {
  return http.get<DeviceStatsDto>('/stats/devices', {
    params: {
      ...filters,
    },
  });
};
