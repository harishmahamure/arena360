import { type DeductionProfile, PlanType, type PlanTypeValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export type { PlanTypeValue };
export { PlanType };

export interface PlanResponse {
  id: string;
  name: string;
  description?: string;
  price: string;
  planType: PlanTypeValue;
  validityDays: number;
  timeWindowStart?: string;
  timeWindowEnd?: string;
  timeCredits?: number;
  isActive: boolean;
  deviceSubType?: string;
  deviceType?: string;
  allowedDays?: string[];
  allowedMonths?: number[];
  dynamicDeductionEnabled?: boolean;
  deductionProfile?: DeductionProfile | null;
  createdAt: string;
  updatedAt: string;
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
  planType?: PlanTypeValue;
  isActive?: 0 | 1;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getPlans = async (filters?: GetPlansFilters) => {
  return http.get<GetPlansResponse>('/plans', {
    params: filters,
  });
};
