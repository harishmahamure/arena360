import { http } from '@gaming-cafe/utils';

export const deletePlan = async (id: string) => {
  return http.delete(`/plans/${id}`);
};
