import { http } from '@gaming-cafe/utils';

export type StaffGamingAllowanceStatus = 'active' | 'expired' | 'exhausted' | 'none';

export interface StaffGamingAllowanceSummary {
  userId: string;
  status: StaffGamingAllowanceStatus;
  allottedMinutes: number;
  remainingMinutes: number;
  usedMinutes: number;
  periodStart?: string;
  periodEnd?: string;
  balanceId?: string;
}

export const getStaffGamingAllowance = async (userId: string) =>
  http.get<StaffGamingAllowanceSummary>(`/users/${userId}/staff-gaming-allowance`);
