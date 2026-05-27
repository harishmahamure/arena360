import { http } from '@gaming-cafe/utils';

interface AddGameRequest {
  title: string;
  description?: string;
  genre?: string;
  category?: string;
  platform?: string;
  isActive?: boolean;
  imageUrl?: string;
  videoUrl?: string;
  trailerUrl?: string;
  developer?: string;
  publisher?: string;
  releaseDate?: string;
  isMultiplayer?: boolean;
  tags?: string[];
  ageRating?: string;
  minPlayers?: number;
  maxPlayers?: number;
}

export const addGame = async (game: AddGameRequest) => {
  return http.post('/games', game);
};
