import { http } from '@gaming-cafe/utils';
import type { PlanResponse } from './list';

export const getPlanById = async (id: string) => {
  return http.get<PlanResponse>(`/plans/${id}`);
};
