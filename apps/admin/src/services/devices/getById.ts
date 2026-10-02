import { http } from '@gaming-cafe/utils';
import type { DeviceResponse } from './list';

export const getDeviceById = async (id: string) => {
  return http.get<DeviceResponse>(`/devices/${id}`);
};
