import { http } from '@gaming-cafe/utils';

interface UpdateDeviceGameRequest {
  installationDate?: string;
  isActive?: boolean;
}

export const updateDeviceGame = async (id: string, data: UpdateDeviceGameRequest) => {
  return http.patch(`/device-games/${id}`, data);
};

export const toggleDeviceGameActive = async (id: string, isActive: boolean) => {
  return http.patch(`/device-games/${id}`, { isActive });
};
