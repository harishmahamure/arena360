import { http } from '@gaming-cafe/utils';

export const deleteSession = async (id: string) => {
  return http.delete(`/sessions/${id}`);
};
