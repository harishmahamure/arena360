import { useQuery } from '@tanstack/react-query';
import { getActiveShift } from '../services/shifts';
import { getStaffDashboardStats } from '../services/stats/getStaffDashboardStats';

export const useStaffDashboardStats = () => {
  const { data: activeShift } = useQuery({
    queryKey: ['activeShift'],
    queryFn: getActiveShift,
    retry: false,
  });

  const shiftStart = activeShift?.clockIn;

  return useQuery({
    queryKey: ['staffDashboardStats', shiftStart],
    queryFn: () =>
      getStaffDashboardStats({
        shiftStart,
      }),
  });
};
