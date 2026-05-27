import { useQuery } from '@tanstack/react-query';
import { getStaffShiftStart } from '../constants/staffShift';
import { getStaffDashboardStats } from '../services/stats/getStaffDashboardStats';
import type { StaffStatsQueryDto } from '../services/stats/types';

export const useStaffDashboardStats = (filters?: Omit<StaffStatsQueryDto, 'shiftStart'>) => {
  const shiftStart = getStaffShiftStart();

  return useQuery({
    queryKey: ['staffDashboardStats', filters, shiftStart],
    queryFn: () =>
      getStaffDashboardStats({
        ...filters,
        shiftStart,
      }),
  });
};
