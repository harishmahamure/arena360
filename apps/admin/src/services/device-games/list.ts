import { http } from '@gaming-cafe/utils';

export interface DeviceGameDevice {
  id: string;
  name: string;
  deviceType: string;
  location?: string;
}

export interface DeviceGameGame {
  id: string;
  title: string;
  genre?: string;
}

export interface DeviceGameResponse {
  id: string;
  deviceId: string;
  gameId: string;
  installationDate: string;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  device?: DeviceGameDevice;
  game?: DeviceGameGame;
}

interface GetDeviceGamesResponse {
  data: DeviceGameResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetDeviceGamesFilters {
  deviceId?: string;
  gameId?: string;
  isActive?: 0 | 1;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getDeviceGames = async (filters?: GetDeviceGamesFilters) => {
  return http.get<GetDeviceGamesResponse>('/device-games', {
    params: filters,
  });
};

// Get all games assigned to a specific device
export const getGamesByDevice = async (deviceId: string) => {
  return http.get<GetDeviceGamesResponse>(`/device-games/device/${deviceId}`);
};

// Get all devices that have a specific game installed
export const getDevicesByGame = async (gameId: string) => {
  return http.get<GetDeviceGamesResponse>(`/device-games/game/${gameId}`);
};
