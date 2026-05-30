import { http } from '@gaming-cafe/utils';

export const deleteGame = async (id: string) => {
  return http.delete(`/games/${id}`);
};
