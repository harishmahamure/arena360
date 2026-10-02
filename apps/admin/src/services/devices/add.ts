import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';
import type { DeviceResponse } from './list';

export interface AddDeviceRequest {
  name: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType: string;
  deviceSubType: string;
  location?: string;
  status?: DeviceStatusValue;
}

export const addDevice = async (device: AddDeviceRequest) => {
  return http.post<DeviceResponse>('/devices', device);
};
