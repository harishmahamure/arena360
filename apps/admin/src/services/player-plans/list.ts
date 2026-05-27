import { http } from '@gaming-cafe/utils';

export enum PlayerPlanStatus {
  ACTIVE = 'active',
  EXPIRED = 'expired',
  EXHAUSTED = 'exhausted',
  CANCELLED = 'cancelled',
  SUSPENDED = 'suspended',
}

export interface PlayerPlanResponse {
  id: string;
  playerId: string;
  planId: string;
  purchaseDate: string;
  activationDate?: string | null;
  expiryDate: string;
  remainingUsageCount?: number | null;
  remainingTimeCredits?: number | null;
  status: string;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
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
  planId?: string;
  status?: PlayerPlanStatus | string;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getPlayerPlans = async (filters: GetPlayerPlansFilters = {}) => {
  return http.get<GetPlayerPlansResponse>('/player-plans', {
    params: {
      ...filters,
      sortBy: filters.sortBy || 'purchaseDate',
      sortOrder: filters.sortOrder || 'DESC',
      limit: filters.limit || 100,
      page: filters.page || 1,
    },
  });
};
