import { http } from '@gaming-cafe/utils';
import type { GameResponse } from './list';

export interface GamePayload {
  name: string;
  thumbnailUrl?: string | null;
  logoUrl?: string | null;
  videoUrl?: string | null;
  launchRef?: string | null;
  isActive?: boolean;
  sortOrder?: number;
}

export const addGame = async (payload: GamePayload) => {
  return http.post<GameResponse>('/games', payload);
};
