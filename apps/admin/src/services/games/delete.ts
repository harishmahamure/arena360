import { http } from '@gaming-cafe/utils';

export const deleteGame = async (id: string) => {
  try {
    return http.patch(`/games/${id}/deactivate`);
  } catch (_error) {
    throw new Error('Failed to delete game');
  }
};
