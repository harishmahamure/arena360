import { http } from '@gaming-cafe/utils';
import type { PlayerResponse } from './list';

export const getPlayerById = async (id: string) => {
  return http.get<PlayerResponse>(`/users/${id}`);
};
