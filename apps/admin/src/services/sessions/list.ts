import type { DeductionProfile } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export interface SessionPlayerSummary {
  id: string;
  username: string;
  firstName?: string | null;
  lastName?: string | null;
}

export interface SessionPlanSummary {
  id: string;
  name: string;
  planType: string;
  timeCredits: number;
}

export interface SessionBalanceSummary {
  id: string;
  playerId: string;
  kind: string;
  remainingMinutes: number;
  status: string;
  expiryDate?: string;
  deductionProfile?: DeductionProfile | null;
  player?: SessionPlayerSummary | null;
  plan?: SessionPlanSummary | null;
}

export interface SessionDeviceSummary {
  id: string;
  name: string;
  deviceType: string;
  location?: string | null;
  status: string;
}

export interface SessionResponse {
  id: string;
  balanceId: string | null;
  deviceId: string;
  startTime: string;
  endTime?: string | null;
  durationMinutes?: number | null;
  timeCreditsConsumed?: number | null;
  walletMinutesAtStart?: number | null;
  sourcePlanIdAtStart?: string | null;
  planAtStart?: SessionPlanSummary | null;
  cafeTimezone?: string;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
  balance?: SessionBalanceSummary | null;
  device?: SessionDeviceSummary | null;
}

export interface GetSessionsResponseData {
  data: SessionResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export const getSessions = async (filters: Record<string, unknown> = {}) => {
  return http.get<GetSessionsResponseData>('/sessions', {
    params: {
      ...filters,
      sortBy: filters.sortBy || 'startTime',
      sortOrder: filters.sortOrder || 'DESC',
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });
};
