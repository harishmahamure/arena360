import { http } from '@gaming-cafe/utils';

export const deletePlayer = async (id: string) => {
  try {
    return http.put(`/users/${id}`, { isActive: false });
  } catch (_error) {
    throw new Error('Failed to deactivate player');
  }
};

export const deactivatePlayer = async (id: string) => {
  try {
    return http.put(`/users/${id}`, { isActive: false });
  } catch (_error) {
    throw new Error('Failed to deactivate player');
  }
};
