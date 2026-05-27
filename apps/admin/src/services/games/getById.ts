import { http } from '@gaming-cafe/utils';
import type { GameResponse } from './list';

export const getGameById = async (id: string) => {
  return http.get<GameResponse>(`/games/${id}`);
};
