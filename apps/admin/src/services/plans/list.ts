import { http } from '@gaming-cafe/utils';

export enum PlanType {
  TIME_BASED = 'time_based',
  SESSION_BASED = 'session_based',
  UNLIMITED_DAILY = 'unlimited_daily',
  HOURLY_RENTAL = 'hourly_rental',
  MONTHLY_SUBSCRIPTION = 'monthly_subscription',
  WEEKEND_SPECIAL = 'weekend_special',
}

export interface PlanResponse {
  id: string;
  name: string;
  description?: string;
  price: string;
  planType: PlanType;
  durationMinutes?: number;
  validityDays: number;
  timeWindowStart?: string;
  timeWindowEnd?: string;
  timeCredits?: number;
  perMinuteRate: number;
  maxSessions?: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  deviceSubType?: string;
  deviceType?: string;
}

interface GetPlansResponse {
  data: PlanResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetPlansFilters {
  search?: string;
  planType?: PlanType;
  isActive?: 0 | 1;
  minPrice?: number;
  maxPrice?: number;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getPlans = async (filters: GetPlansFilters = {}) => {
  return http.get<GetPlansResponse>('/plans', {
    params: {
      ...filters,
    },
  });
};
