import { http } from '@gaming-cafe/utils';

export interface GameResponse {
  id: string;
  name: string;
  thumbnailUrl: string | null;
  logoUrl: string | null;
  videoUrl: string | null;
  launchRef: string | null;
  isActive: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

interface GetGamesResponse {
  data: GameResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetGamesFilters {
  name?: string;
  isActive?: boolean;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getGames = async (filters: GetGamesFilters = {}) => {
  return http.get<GetGamesResponse>('/games', { params: filters });
};
