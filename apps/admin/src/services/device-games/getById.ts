import { http } from '@gaming-cafe/utils';
import type { DeviceGameResponse } from './list';

export const getDeviceGameById = async (id: string) => {
  return http.get<DeviceGameResponse>(`/device-games/${id}`);
};
