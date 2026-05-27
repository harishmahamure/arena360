import { http } from '@gaming-cafe/utils';
import type { StaffDashboardStatsDto, StaffStatsQueryDto } from './types';

export const getStaffDashboardStats = async (filters?: StaffStatsQueryDto) => {
  return http.get<StaffDashboardStatsDto>('/stats/staff-dashboard', {
    params: filters,
  });
};
