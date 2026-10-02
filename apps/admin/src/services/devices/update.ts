import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export interface UpdateDeviceRequest {
  name?: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType?: string;
  deviceSubType?: string;
  location?: string;
  status?: DeviceStatusValue;
}

export const updateDevice = async (id: string, device: UpdateDeviceRequest) => {
  return http.patch(`/devices/${id}`, device);
};

export interface UpdateDeviceStatusRequest {
  status: DeviceStatusValue;
}

export const updateDeviceStatus = async (id: string, data: UpdateDeviceStatusRequest) => {
  return http.patch(`/devices/${id}/status`, data);
};
