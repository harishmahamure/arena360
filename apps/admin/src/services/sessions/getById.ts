import { http } from '@gaming-cafe/utils';
import type { SessionResponse } from './list';

export const getSessionById = async (id: string) => {
  return http.get<SessionResponse>(`/sessions/${id}`);
};
