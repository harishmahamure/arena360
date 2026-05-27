import { http } from '@gaming-cafe/utils';
import type { PlanResponse, PlanType } from './list';

export interface CreatePlanPayload {
  name: string;
  description?: string;
  price: number;
  planType: PlanType;
  durationMinutes?: number;
  validityDays?: number;
  timeWindowStart?: string;
  timeWindowEnd?: string;
  timeCredits?: number;
  perMinuteRate?: number;
  maxSessions?: number;
  isActive?: boolean;
}

export const addPlan = async (payload: CreatePlanPayload) => {
  return http.post<PlanResponse>('/plans', payload);
};
