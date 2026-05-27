import { http } from '@gaming-cafe/utils';

export const deleteDevice = async (id: string) => {
  try {
    return http.delete(`/devices/${id}`);
  } catch (_error) {
    throw new Error('Failed to delete device');
  }
};
