import { http } from '@gaming-cafe/utils';
import type { StaffGamingAllowanceSummary } from './getStaffGamingAllowance';

export const setStaffGamingAllowance = async (userId: string, allottedHours: number) =>
  http.patch<StaffGamingAllowanceSummary>(`/users/${userId}/staff-gaming-allowance`, {
    allottedHours,
  });
