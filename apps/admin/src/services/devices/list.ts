import { DeviceStatus, type DeviceStatusValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export { DeviceStatus };

export interface DeviceResponse {
  id: string;
  name: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType: string;
  deviceSubType: string;
  location?: string;
  status: DeviceStatusValue;
  registrationStatus?: string;
  registrationCode?: string | null;
  registrationCodeExpiresAt?: string | null;
  /** Stored hardware fingerprint JSON (mac/serial/biosUuid/...). */
  registeredKiosk?: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string;
}

interface GetDevicesResponse {
  data: DeviceResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetDevicesFilters {
  status?: DeviceStatusValue;
  deviceType?: string;
  location?: string;
  name?: string;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getDevices = async (filters?: GetDevicesFilters) => {
  return http.get<GetDevicesResponse>('/devices', {
    params: filters,
  });
};
