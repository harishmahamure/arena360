import { http } from '@gaming-cafe/utils';
import type { CreateGameFormData } from '../../containers/games/schemas/game-schema';

export const updateGame = async (id: string, game: CreateGameFormData) => {
  // Transform tags from comma-separated string to array if needed
  const payload = {
    ...game,
    tags: game.tags
      ? game.tags
          .split(',')
          .map((tag) => tag.trim())
          .filter(Boolean)
      : undefined,
  };

  return http.put(`/games/${id}`, payload);
};
