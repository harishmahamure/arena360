import { http } from '@gaming-cafe/utils';
import { normalizeRevenue } from './statsHelpers';
import type { StaffDashboardStatsDto, StaffStatsQueryDto } from './types';

export const getStaffDashboardStats = async (filters?: StaffStatsQueryDto) => {
  const data = await http.get<StaffDashboardStatsDto>('/stats/staff-dashboard', {
    params: filters,
  });

  return {
    ...data,
    revenue: normalizeRevenue(data.revenue),
    shiftRevenue: data.shiftRevenue ? normalizeRevenue(data.shiftRevenue) : data.shiftRevenue,
  };
};
