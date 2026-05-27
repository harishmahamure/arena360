import { http } from '@gaming-cafe/utils';

export const deleteUnit = async (id: string) => {
  return http.delete(`/units/${id}`);
};
