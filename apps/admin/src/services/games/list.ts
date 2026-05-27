import { http } from '@gaming-cafe/utils';

interface GetGamesResponse {
  data: GameResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GameResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  title: string;
  description: string | null;
  genre: string | null;
  isActive: boolean;
  imageUrl: string | null;
  videoUrl: string | null;
  trailerUrl: string | null;
  developer: string | null;
  publisher: string | null;
  releaseDate: string | null;
  platform: string | null;
  category: string | null;
  isMultiplayer: boolean;
  tags: string[] | null;
  ageRating: string | null;
  minPlayers: number | null;
  maxPlayers: number | null;
}

export interface GetGamesFilters {
  title?: string;
  genre?: string;
  category?: string;
  platform?: string;
  developer?: string;
  publisher?: string;
  isActive?: 0 | 1;
  isMultiplayer?: 0 | 1;
  ageRating?: string;
  tag?: string;
  page?: number;
  limit?: number;
  sort?: 'asc' | 'desc';
}

export const getGames = async (filters: GetGamesFilters) => {
  return http.get<GetGamesResponse>('/games', {
    params: {
      ...filters,
    },
  });
};
