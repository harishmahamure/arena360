import { http } from '@gaming-cafe/utils';

interface AssignGameToDeviceRequest {
  deviceId: string;
  gameId: string;
  installationDate?: string;
  isActive?: boolean;
}

export const assignGameToDevice = async (data: AssignGameToDeviceRequest) => {
  return http.post('/device-games', data);
};
