import { http } from '@gaming-cafe/utils';

export const deleteGame = async (id: string) => {
  try {
    return http.delete(`/games/${id}`);
  } catch (_error) {
    throw new Error('Failed to deactivate game');
  }
};
