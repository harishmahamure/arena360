import { http } from '@gaming-cafe/utils';

export const removeGameFromDevice = async (id: string) => {
  try {
    return http.delete(`/device-games/${id}`);
  } catch (_error) {
    throw new Error('Failed to remove game from device');
  }
};

// Deactivate instead of delete (soft delete)
export const deactivateDeviceGame = async (id: string) => {
  try {
    return http.patch(`/device-games/${id}`, {
      isActive: false,
    });
  } catch (_error) {
    throw new Error('Failed to deactivate device game assignment');
  }
};
