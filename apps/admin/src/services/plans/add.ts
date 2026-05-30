import { http } from '@gaming-cafe/utils';
import type { PlanResponse, PlanTypeValue } from './list';

export interface CreatePlanPayload {
  name: string;
  description?: string;
  price: number;
  planType: PlanTypeValue;
  validityDays?: number;
  timeWindowStart?: string;
  timeWindowEnd?: string;
  timeCredits?: number;
  isActive?: boolean;
  deviceType?: string;
  deviceSubType?: string;
  allowedDays?: string[];
  allowedMonths?: number[];
}

export const addPlan = async (payload: CreatePlanPayload) => {
  return http.post<PlanResponse>('/plans', payload);
};
