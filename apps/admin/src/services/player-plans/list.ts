import { http } from '@gaming-cafe/utils';

export enum PlayerPlanStatus {
  ACTIVE = 'active',
  EXPIRED = 'expired',
  SUSPENDED = 'suspended',
}

export interface PlayerPlanResponse {
  id: string;
  playerId: string;
  planId: string;
  status: PlayerPlanStatus;
  remainingTimeCredits?: number;
  expiresAt?: string;
  createdAt: string;
  updatedAt: string;
  player?: {
    id: string;
    email: string;
    username: string;
    firstName?: string;
    lastName?: string;
  };
  plan?: {
    id: string;
    name: string;
    description?: string;
    planType: string;
    price: string;
    timeCredits?: number;
  };
}

interface GetPlayerPlansResponse {
  data: PlayerPlanResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetPlayerPlansFilters {
  playerId?: string;
  planId?: string;
  status?: PlayerPlanStatus;
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
      limit: filters.limit || 100, // Get more for dropdown
      page: filters.page || 1,
    },
  });
};
