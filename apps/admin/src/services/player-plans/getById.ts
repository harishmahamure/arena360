import { http } from '@gaming-cafe/utils';
import type { PlayerPlanResponse } from './list';

export const getPlayerPlanById = async (id: string) => {
  return http.get<PlayerPlanResponse>(`/player-plans/${id}`);
};
