import {
  type DeductionProfile,
  PlayerPlanStatus,
  type PlayerPlanStatusValue,
} from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export { PlayerPlanStatus };

export interface PlayerPlanResponse {
  id: string;
  playerId: string;
  deviceType?: string | null;
  deviceSubType?: string | null;
  kind: string;
  remainingMinutes: number;
  expiryDate: string;
  windowStart?: string | null;
  windowEnd?: string | null;
  status: string;
  sourcePlanId?: string | null;
  createdBy?: string | null;
  updatedBy?: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
  deductionProfile?: DeductionProfile | null;
  player?: {
    id: string;
    username: string;
    firstName?: string | null;
    lastName?: string | null;
  } | null;
  plan?: {
    id: string;
    name: string;
    planType: string;
    price: number;
    timeCredits: number;
  } | null;
}

export interface GetPlayerPlansResponse {
  data: PlayerPlanResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetPlayerPlansFilters {
  playerId?: string;
  kind?: string;
  status?: PlayerPlanStatusValue | string;
  deviceType?: string;
  deviceSubType?: string;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getPlayerPlans = async (filters: GetPlayerPlansFilters = {}) => {
  return http.get<GetPlayerPlansResponse>('/player-plans', {
    params: {
      ...filters,
      sortBy: filters.sortBy || 'createdAt',
      sortOrder: filters.sortOrder || 'DESC',
      limit: filters.limit || 100,
      page: filters.page || 1,
    },
  });
};
