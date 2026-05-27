import { http } from '@gaming-cafe/utils';

// Enums for device
export enum DeviceStatus {
  OPERATIONAL = 'operational',
  UNDER_MAINTENANCE = 'under_maintenance',
  OUT_OF_SERVICE = 'out_of_service',
  IN_USE = 'in_use',
  AVAILABLE = 'available',
}
export interface DeviceResponse {
  id: string;
  name: string;
  serialNumber?: string;
  localIpAddress?: string;
  deviceType: string;
  location?: string;
  status: DeviceStatus;
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
  status?: DeviceStatus;
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
