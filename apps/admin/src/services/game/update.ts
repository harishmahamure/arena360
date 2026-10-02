import { http } from '@gaming-cafe/utils';
import type { GamePayload } from './add';
import type { GameResponse } from './list';

export const updateGame = async (id: string, payload: Partial<GamePayload>) => {
  return http.patch<GameResponse>(`/games/${id}`, payload);
};
