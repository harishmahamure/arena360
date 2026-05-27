import { http } from '@gaming-cafe/utils';
import type { DeviceStatus } from './list';

export interface AddDeviceRequest {
  name: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType: string;
  location?: string;
  status?: DeviceStatus;
}

export const addDevice = async (device: AddDeviceRequest) => {
  return http.post('/devices', device);
};
