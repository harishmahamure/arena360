import { http } from '@gaming-cafe/utils';

export const deletePlayer = async (id: string) => {
  try {
    return http.delete(`/users/${id}`);
  } catch (_error) {
    throw new Error('Failed to delete player');
  }
};

export const deactivatePlayer = async (id: string) => {
  try {
    return http.patch(`/users/${id}`, { isActive: false });
  } catch (_error) {
    throw new Error('Failed to deactivate player');
  }
};
