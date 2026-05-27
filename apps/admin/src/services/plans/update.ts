import { http } from '@gaming-cafe/utils';
import type { CreatePlanPayload } from './add';
import type { PlanResponse } from './list';

export const updatePlan = async (id: string, payload: Partial<CreatePlanPayload>) => {
  return http.patch<PlanResponse>(`/plans/${id}`, payload);
};
