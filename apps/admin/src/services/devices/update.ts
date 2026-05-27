import { http } from '@gaming-cafe/utils';
import type { DeviceStatus } from './list';

export interface UpdateDeviceRequest {
  name?: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType?: string;
  location?: string;
  status?: DeviceStatus;
}

export const updateDevice = async (id: string, device: UpdateDeviceRequest) => {
  return http.patch(`/devices/${id}`, device);
};

export interface UpdateDeviceStatusRequest {
  status: DeviceStatus;
}

export const updateDeviceStatus = async (id: string, data: UpdateDeviceStatusRequest) => {
  return http.patch(`/devices/${id}/status`, data);
};
